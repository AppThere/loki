// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Microbenchmark of the document open → layout-ready pipeline.
//!
//! Times each CPU stage of opening the acid DOCX fixture so the open-latency
//! hotspots are measurable headlessly and repeatably:
//!
//! ```text
//! cargo run -p loki-acid --release --example load_bench
//! ```
//!
//! Each stage is run several times; the **median** and **min** are reported so
//! a single GC/scheduler hiccup does not skew the picture. Two composite totals
//! frame the two real cases the editor hits:
//!
//! * **Cold open** — the first document opened in a process: a fresh system-font
//!   scan (`FontResources::new`) plus import, Loro seed, and the first layout.
//! * **Warm reopen** — every subsequent open reuses the editor's per-document
//!   `FontResources`, so the font scan is *not* repaid; only import, Loro seed,
//!   and layout remain.
//!
//! GPU first-paint (page-tile upload + shader warm-up) is *not* covered here —
//! it cannot run headless. Measure it on-device via the `loki_text::open` tracing
//! spans added to the real editor open path (see `editor_load`/`editing::state`).

use std::io::Cursor;
use std::time::{Duration, Instant};

use loki_doc_model::io::DocumentImport;
use loki_doc_model::loro_bridge::document_to_loro;
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_ooxml::{DocxImport, DocxImportOptions};

/// Iterations per stage. Small, since each layout pass is tens of ms.
const ITERS: usize = 9;

/// Runs `f` `ITERS` times and returns `(median, min)` of the durations it
/// reports. The closure returns the span it wants timed, so per-iteration setup
/// (e.g. building a fresh, cold `FontResources`) can be excluded from the sample.
fn bench(mut f: impl FnMut() -> Duration) -> (Duration, Duration) {
    let mut samples: Vec<Duration> = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        samples.push(f());
    }
    samples.sort_unstable();
    (samples[samples.len() / 2], samples[0])
}

/// Times a single call to `f`, returning how long it took.
fn timed(f: impl FnOnce()) -> Duration {
    let t = Instant::now();
    f();
    t.elapsed()
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn row(label: &str, median: Duration, min: Duration) {
    eprintln!(
        "  {label:<32} median {:>7.1} ms   min {:>7.1} ms",
        ms(median),
        ms(min)
    );
}

fn main() {
    let bytes = loki_acid::Fixture::Docx.bytes();

    // Reference parse for the page count / structural summary.
    let doc =
        DocxImport::import(Cursor::new(&bytes), DocxImportOptions::default()).expect("import docx");
    let opts = LayoutOptions {
        preserve_for_editing: true,
    };
    let pages = {
        let mut fonts = FontResources::new();
        match layout_document(&mut fonts, &doc, LayoutMode::Paginated, 1.0, &opts) {
            loki_layout::DocumentLayout::Paginated(p) => p.pages.len(),
            _ => 0,
        }
    };

    eprintln!(
        "load_bench — acid_docx.docx ({} KiB, {} pages, {ITERS} iters/stage)\n",
        bytes.len() / 1024,
        pages,
    );

    // ── Stage 1: system-font scan ────────────────────────────────────────────
    // Paid once per process by the first DocumentState; reused thereafter.
    let (scan_med, scan_min) = bench(|| timed(|| drop(FontResources::new())));
    row("FontResources::new (font scan)", scan_med, scan_min);

    // ── Stage 2: DOCX import ─────────────────────────────────────────────────
    let (imp_med, imp_min) = bench(|| {
        timed(|| {
            DocxImport::import(Cursor::new(&bytes), DocxImportOptions::default()).expect("import");
        })
    });
    row("DocxImport::import", imp_med, imp_min);

    // ── Stage 3: Loro CRDT seed ──────────────────────────────────────────────
    let (loro_med, loro_min) = bench(|| {
        timed(|| {
            document_to_loro(&doc).expect("document_to_loro");
        })
    });
    row("document_to_loro", loro_med, loro_min);

    // ── Stage 4a: FIRST paginated layout on a cold FontResources ─────────────
    // This is what `seed_layout_from_document` pays on open: the very first
    // layout against a freshly-built FontResources, whose Parley shaping caches,
    // font-data cache, and glyph scratch are all cold. The fresh FontResources
    // is built *outside* the timed span (its scan is Stage 1), so this isolates
    // the cold-cache layout cost — the dominant term in open latency.
    let (cold_lay_med, cold_lay_min) = bench(|| {
        let mut fonts = FontResources::new();
        timed(|| {
            layout_document(&mut fonts, &doc, LayoutMode::Paginated, 1.0, &opts);
        })
    });
    row("layout_document (cold, first)", cold_lay_med, cold_lay_min);

    // ── Stage 4b: relayout on a WARM FontResources ───────────────────────────
    // Steady-state cost once the editor's FontResources is warm — e.g. the
    // relayout after a keystroke. Prime once, then time fresh layouts with only
    // the paragraph cache cleared (as `seed_layout_from_document` does).
    let mut warm_fonts = FontResources::new();
    let _ = layout_document(&mut warm_fonts, &doc, LayoutMode::Paginated, 1.0, &opts);
    let (warm_lay_med, warm_lay_min) = bench(|| {
        warm_fonts.clear_paragraph_cache();
        timed(|| {
            layout_document(&mut warm_fonts, &doc, LayoutMode::Paginated, 1.0, &opts);
        })
    });
    row(
        "layout_document (warm, relayout)",
        warm_lay_med,
        warm_lay_min,
    );

    // ── Composite total (median-based) ───────────────────────────────────────
    // The cold open is what the user feels opening the first document: font
    // scan + import + Loro seed + the cold first layout.
    let cold_open = scan_med + imp_med + loro_med + cold_lay_med;
    eprintln!();
    eprintln!(
        "  {:<32} {:>7.1} ms   (scan + import + loro + cold layout)",
        "Cold open (first document, CPU)",
        ms(cold_open),
    );
    eprintln!(
        "\n  Cold first layout is {:.0}% of the cold open — it is the hotspot.",
        ms(cold_lay_med) / ms(cold_open) * 100.0,
    );
    eprintln!(
        "  Warm relayout is ~{:.0}x cheaper, so the cost is one-time cache warm-up,",
        ms(cold_lay_med) / ms(warm_lay_med).max(0.01),
    );
    eprintln!("  not the layout algorithm itself. GPU first-paint is additional —");
    eprintln!("  measure it on-device via the loki_text::open tracing spans.");
}
