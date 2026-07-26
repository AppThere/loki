// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Spec 09 E0** — how much resident memory is editing residency?
//!
//! Spec 09 gates its whole phase plan (L9-005) on one experiment: does turning
//! `preserve_for_editing` off actually recover the ~72 bytes per character that
//! `docs/spikes/S09.0-layout-residency-census.md` predicts from struct
//! definitions? If not, the census is wrong and everything built on it is built
//! on sand.
//!
//! # Why this is a bench and not a manual RSS comparison
//!
//! Spec 09 §4 proposes opening a document read-only, opening it for editing,
//! and diffing RSS across two process runs — with caveats about forcing a full
//! layout pass and about allocator retention masking the difference.
//!
//! None of that is necessary. **Layout is CPU-only** (Parley shaping plus our
//! pagination; the GPU is involved in painting, not in layout), so the
//! comparison runs headless, and dhat measures the region directly:
//!
//! - the full layout pass is forced by construction — we call
//!   `layout_document` and hold the result, so there is no lazy pagination to
//!   confound;
//! - allocator retention cannot mask anything, because dhat counts live heap
//!   bytes rather than what the allocator has returned to the OS;
//! - it is repeatable and diffable, so it also serves as the regression guard
//!   for S9-1 … S9-4 rather than being spent once.
//!
//! # What the numbers mean
//!
//! `peak_bytes` is the peak *live* heap during the region. The layout is held
//! live at that peak, so the delta between the two conditions is the retained
//! editing data — the quantity Spec 09 proposes to window.
//!
//! `preserve_for_editing: false` still *builds* each Parley layout (line
//! breaking needs it) and drops it per paragraph, so the false condition
//! carries at most one paragraph's shaping at a time while the true condition
//! accumulates all of them. The delta is therefore retention, not shaping work.
//!
//! Run: `cargo bench -p loki-bench --bench layout_editing_residency`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{AllocStats, measure};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use std::hint::black_box;

/// Counts the characters of text in a document, so residency can be reported
/// per character and compared against the census's ~72 B/char prediction.
fn char_count(doc: &Document) -> usize {
    fn inline_chars(i: &Inline) -> usize {
        match i {
            Inline::Str(s) => s.chars().count(),
            Inline::Strong(v) | Inline::Emph(v) => v.iter().map(inline_chars).sum(),
            _ => 0,
        }
    }
    doc.sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .map(|b| match b {
            Block::Para(inlines) | Block::Plain(inlines) => {
                inlines.iter().map(inline_chars).sum::<usize>()
            }
            _ => 0,
        })
        .sum()
}

fn layout_peak(resources: &mut FontResources, doc: &Document, preserve: bool) -> AllocStats {
    let options = LayoutOptions {
        preserve_for_editing: preserve,
        spell: None,
        ..Default::default()
    };
    // Cold paragraph cache in both conditions, or the second run measures a
    // cache hit and the comparison is meaningless.
    resources.clear_paragraph_cache();
    measure(|| {
        let layout = layout_document(resources, doc, LayoutMode::Paginated, 1.0, &options);
        // Held live across the peak — this is the resident set being measured,
        // not the transient cost of producing it.
        black_box(&layout);
    })
}

fn main() {
    support::header("Spec 09 E0 — editing residency: preserve_for_editing on vs off");
    eprintln!(
        "  Predicted by S09.0: ~72 B/char retained for editing, ~88 B/char total.\n\
           A large divergence here invalidates the census (Spec 09 L9-005)."
    );

    let mut resources = FontResources::new();
    let mut worst_delta = 0_i64;

    for &(name, paras) in support::DOC_TIERS {
        let doc = support::build_doc(paras, support::WORDS_PER_PARA);
        let chars = char_count(&doc);

        let editing = layout_peak(&mut resources, &doc, true);
        let read_only = layout_peak(&mut resources, &doc, false);

        support::report_row(&format!("{name} ({paras}p) editing"), editing);
        support::report_row(&format!("{name} ({paras}p) read-only"), read_only);

        let delta = editing.max_bytes as i64 - read_only.max_bytes as i64;
        let per_char = if chars > 0 {
            delta as f64 / chars as f64
        } else {
            0.0
        };
        let total_per_char = if chars > 0 {
            editing.max_bytes as f64 / chars as f64
        } else {
            0.0
        };
        eprintln!(
            "  {:<26} chars={chars:>8}  retained={delta:>11} B  \
             editing={per_char:>6.1} B/char  total={total_per_char:>6.1} B/char",
            format!("{name} ({paras}p) E0"),
        );
        worst_delta = worst_delta.max(delta);
    }

    // The experiment is only meaningful if the two conditions differ at all. A
    // zero (or negative) delta means `preserve_for_editing` is not the switch
    // the census assumes it is, which is itself the finding — fail loudly
    // rather than reporting a tidy zero.
    assert!(
        worst_delta > 0,
        "E0: preserve_for_editing recovered no memory at any tier — \
         the S09.0 census is wrong about what the flag controls"
    );
}
