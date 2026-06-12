// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared synthetic-document builder for the `loki-layout` benchmark harness.
//!
//! Lives in a subdirectory (`benches/support/`) rather than a top-level file so
//! Cargo does *not* auto-discover it as a benchmark target — it is `#[path]`-
//! included by each bench and by the `layout_report` example instead.
//!
//! The builder produces documents of a caller-chosen paragraph count with a
//! realistic mix of plain / bold / italic runs, so layout cost scales with the
//! same kind of content the editor lays out per keystroke. Text is generated
//! deterministically (no RNG) so successive runs are comparable.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;

/// A small fixed word pool — cycling through it gives varied line-break points
/// without pulling in a Lorem-ipsum dependency.
const WORDS: &[&str] = &[
    "loki",
    "document",
    "layout",
    "reflow",
    "paragraph",
    "cursor",
    "glyph",
    "render",
    "office",
    "fidelity",
    "shaping",
    "parley",
    "vello",
    "cluster",
    "baseline",
    "ascent",
    "descent",
    "kerning",
    "ligature",
    "hyphen",
    "justify",
    "indent",
    "margin",
    "column",
];

/// Builds a single paragraph of roughly `words` words, splitting it into three
/// runs (plain, bold, italic) so each paragraph carries multiple style spans —
/// exercising the per-run shaping path rather than one homogeneous run.
fn paragraph(seed: usize, words: usize) -> Block {
    let take = |start: usize, count: usize| -> String {
        let mut s = String::new();
        for i in 0..count {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(WORDS[(start + i) % WORDS.len()]);
        }
        s
    };

    // Split the word budget into three spans: plain ~50%, bold ~25%, italic ~25%.
    let bold = (words / 4).max(1);
    let italic = (words / 4).max(1);
    let plain = words.saturating_sub(bold + italic).max(1);

    let inlines = vec![
        Inline::Str(format!("{}. ", seed + 1)),
        Inline::Str(take(seed, plain)),
        Inline::Str(" ".into()),
        Inline::Strong(vec![Inline::Str(take(seed + plain, bold))]),
        Inline::Str(" ".into()),
        Inline::Emph(vec![Inline::Str(take(seed + plain + bold, italic))]),
    ];
    Block::Para(inlines)
}

/// Builds a [`Document`] with `paras` paragraphs of `words_per_para` words each,
/// in a single section using the default page layout.
///
/// This is the synthetic workload the benchmark harness lays out at varying
/// sizes to characterise how layout / edit-path cost scales with document
/// length.
pub fn build_doc(paras: usize, words_per_para: usize) -> Document {
    let blocks: Vec<Block> = (0..paras).map(|i| paragraph(i, words_per_para)).collect();
    let section = Section::with_layout_and_blocks(PageLayout::default(), blocks);
    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

/// The paragraph counts every bench / report sweeps over. Chosen to span a
/// blank doc up to a ~40-page document so super-linear growth is visible.
pub const SWEEP: &[usize] = &[10, 50, 100, 250, 500, 1000];

/// Words per paragraph used across the harness (a typical body paragraph).
pub const WORDS_PER_PARA: usize = 60;
