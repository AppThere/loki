// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Quick layout / edit-path scaling report.
//!
//! Prints a plain table of layout and per-keystroke cost vs. paragraph count so
//! the O(n)-per-keystroke behaviour is visible at a glance, without waiting for
//! a full Criterion statistical run. Use the Criterion benches
//! (`cargo bench -p loki-layout`) for rigorous numbers; use this for a fast
//! sanity readout on a given machine (e.g. a target device).
//!
//! Run: `cargo run -p loki-layout --example layout_report --release`
//!
//! The `µs/para` columns are the headline: if cost-per-paragraph is roughly
//! flat, every keystroke is paying to recompute the whole document.

use std::time::{Duration, Instant};

use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::loro_mutation::{delete_text, insert_text};
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};

#[path = "../benches/support/mod.rs"]
mod support;

/// Reflow viewport width in points (narrow content column).
const REFLOW_WIDTH_PT: f32 = 360.0;
/// Samples per measurement; the median is reported to shrug off scheduler noise.
const SAMPLES: usize = 7;

/// Runs `f` `SAMPLES` times and returns the median wall-clock duration.
fn median<F: FnMut()>(mut f: F) -> Duration {
    let mut times: Vec<Duration> = (0..SAMPLES)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();
    times.sort_unstable();
    times[times.len() / 2]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000.0
}

fn main() {
    let options = LayoutOptions {
        preserve_for_editing: true,
    };

    // The font scan the renderer currently repeats per generation.
    let font_new = median(|| {
        let _ = FontResources::new();
    });

    // Shared instance for all measured work below — the editor's pattern.
    let mut resources = FontResources::new();

    println!("Loki layout / edit-path scaling report");
    println!(
        "  words/para = {}, samples = {SAMPLES} (median)",
        support::WORDS_PER_PARA
    );
    println!(
        "  FontResources::new() = {:.1} ms  (charged per generation on the renderer path)",
        ms(font_new)
    );
    println!();
    println!(
        "{:>6} | {:>10} {:>9} | {:>10} {:>9} | {:>12} {:>9}",
        "paras", "paginate", "µs/para", "reflow", "µs/para", "keystroke", "µs/para"
    );
    println!("{}", "-".repeat(78));

    for &n in support::SWEEP {
        let doc = support::build_doc(n, support::WORDS_PER_PARA);

        let paginate = median(|| {
            let _ = layout_document(&mut resources, &doc, LayoutMode::Paginated, 1.0, &options);
        });
        let reflow = median(|| {
            let _ = layout_document(
                &mut resources,
                &doc,
                LayoutMode::Reflow {
                    available_width: REFLOW_WIDTH_PT,
                },
                1.0,
                &options,
            );
        });

        // Full per-keystroke pipeline: mutate -> derive -> re-layout -> undo.
        let loro = document_to_loro(&doc).expect("document_to_loro");
        let keystroke = median(|| {
            let _ = insert_text(&loro, 0, 0, "x");
            if let Ok(derived) = loro_to_document(&loro) {
                let _ = layout_document(
                    &mut resources,
                    &derived,
                    LayoutMode::Paginated,
                    1.0,
                    &options,
                );
            }
            let _ = delete_text(&loro, 0, 0, 1);
        });

        let per = |d: Duration| ms(d) * 1_000.0 / n as f64; // µs per paragraph
        println!(
            "{:>6} | {:>9.2}ms {:>9.1} | {:>9.2}ms {:>9.1} | {:>11.2}ms {:>9.1}",
            n,
            ms(paginate),
            per(paginate),
            ms(reflow),
            per(reflow),
            ms(keystroke),
            per(keystroke),
        );
    }

    println!();
    println!("Reading: roughly-flat µs/para across rows => cost grows ~linearly with");
    println!("document length, i.e. every keystroke recomputes the whole document.");
}
