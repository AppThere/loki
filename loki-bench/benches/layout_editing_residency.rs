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
//! It does, for body text — measured 70.1 B/char against 72 predicted, flat
//! across a 4× document-size change. On **real** documents the per-character
//! rate is the wrong unit (71 → 3950 B/char across the corpus) while the
//! *evictable fraction* stays in a 49–74% band. Watch the fraction in the last
//! column, not the rate: that is the number Spec 09 targets (S09.0 §10a).
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
use loki_doc_model::content::toc::inline_plain_text;
use loki_doc_model::document::Document;
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use std::hint::black_box;

/// Counts the characters of display text in a document, so residency can be
/// reported per character.
///
/// Built on `inline_plain_text`, which already flattens every inline variant,
/// rather than matching on a subset — a hand-rolled counter silently returned
/// zero for every real document in the corpus, because they use `StyledPara`
/// and `Heading` where the synthetic builder uses `Para`.
fn char_count(doc: &Document) -> usize {
    fn block_chars(b: &Block) -> usize {
        match b {
            Block::Para(i) | Block::Plain(i) => inline_plain_text(i).chars().count(),
            Block::Heading(_, _, i) => inline_plain_text(i).chars().count(),
            Block::StyledPara(p) => inline_plain_text(&p.inlines).chars().count(),
            Block::BlockQuote(inner) => inner.iter().map(block_chars).sum(),
            Block::OrderedList(_, items) | Block::BulletList(items) => items
                .iter()
                .flat_map(|blocks| blocks.iter())
                .map(block_chars)
                .sum(),
            Block::Table(t) => table_chars(t),
            _ => 0,
        }
    }
    fn table_chars(t: &loki_doc_model::content::table::Table) -> usize {
        t.head
            .rows
            .iter()
            .chain(
                t.bodies
                    .iter()
                    .flat_map(|b| b.head_rows.iter().chain(b.body_rows.iter())),
            )
            .chain(t.foot.rows.iter())
            .flat_map(|row| row.cells.iter())
            .flat_map(|cell| cell.blocks.iter())
            .map(block_chars)
            .sum()
    }
    doc.sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .map(block_chars)
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

/// Real documents from the conformance corpus, as `(label, relative path)`.
///
/// The synthetic tiers above carry three style runs per paragraph, which is
/// denser than plain prose but far lighter than a real document. Laying out the
/// actual fixtures answers the question the synthetic corpus cannot: does the
/// per-character model survive real formatting density, or do the per-run and
/// per-line terms take over?
///
/// Read from disk by path rather than through `appthere-conformance`, so this
/// bench adds no crate edge for the dependency-direction gate to weigh.
const CORPUS: &[(&str, &str)] = &[
    (
        "acid-docx",
        "../appthere-conformance/fixtures/docx/acid-docx.docx",
    ),
    (
        "acid2-docx",
        "../appthere-conformance/fixtures/docx/acid2-docx.docx",
    ),
    (
        "iris-blueprint",
        "../appthere-conformance/fixtures/docx/iris-blueprint.docx",
    ),
    (
        "styles-tinos",
        "../appthere-conformance/fixtures/odt/styles-tinos.odt",
    ),
    (
        "para-gelasio",
        "../appthere-conformance/fixtures/odt/para-gelasio.odt",
    ),
    (
        "para-carlito",
        "../appthere-conformance/fixtures/odt/para-carlito.odt",
    ),
];

/// Imports a corpus fixture, or `None` when it is absent or fails to import.
///
/// Absence is tolerated rather than fatal: this is a bench, and the synthetic
/// tiers above are the part that gates Spec 09. A missing corpus degrades the
/// run to "no real-document evidence", which the report states plainly.
fn load_corpus_doc(rel: &str) -> Option<Document> {
    use loki_doc_model::io::DocumentImport;
    use std::io::Cursor;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let bytes = std::fs::read(&path).ok()?;
    if rel.ends_with(".docx") {
        loki_ooxml::DocxImport::import(Cursor::new(bytes.as_slice()), Default::default()).ok()
    } else {
        loki_odf::OdtImport::import(Cursor::new(bytes.as_slice()), Default::default()).ok()
    }
}

/// Measures one document and prints its per-character residency.
fn report_doc(resources: &mut FontResources, label: &str, doc: &Document) -> i64 {
    let chars = char_count(doc);
    if chars == 0 {
        eprintln!("  {label:<26} skipped — no text content");
        return 0;
    }
    let editing = layout_peak(resources, doc, true);
    let read_only = layout_peak(resources, doc, false);
    let delta = editing.max_bytes as i64 - read_only.max_bytes as i64;
    eprintln!(
        "  {label:<26} chars={chars:>8}  retained={delta:>11} B  \
         editing={:>6.1} B/char  total={:>6.1} B/char",
        delta as f64 / chars as f64,
        editing.max_bytes as f64 / chars as f64,
    );
    delta
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

    // ── Real documents ──────────────────────────────────────────────────────
    // The synthetic tiers establish the model; these test it against actual
    // formatting density, which is the open question S09.0 §11 item 1 records.
    eprintln!("\n  real documents (conformance corpus):");
    let mut corpus_seen = 0_usize;
    for &(label, rel) in CORPUS {
        match load_corpus_doc(rel) {
            Some(doc) => {
                report_doc(&mut resources, label, &doc);
                corpus_seen += 1;
            }
            None => eprintln!("  {label:<26} unavailable (absent or import failed)"),
        }
    }
    if corpus_seen == 0 {
        eprintln!("  (no corpus documents loaded — synthetic tiers only)");
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
