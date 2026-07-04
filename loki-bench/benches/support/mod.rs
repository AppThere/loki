// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared scale-corpus builders + report formatting for the portable benches
//! (Spec 06 M2 / §8).
//!
//! Lives in `benches/support/` (a subdirectory) so Cargo does *not* auto-detect
//! it as a bench target; each bench `#[path]`-includes it. The generators are
//! deterministic (no RNG) so successive runs are comparable, and the tier presets
//! are the *scale* corpus (size), distinct from Spec 02's feature fixtures.

#![allow(dead_code)] // Each bench uses a different subset of these helpers.

use loki_bench::AllocStats;
use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
use loki_doc_model::style::{ParagraphStyle, StyleCatalog, StyleId};

/// A small fixed word pool — cycling it gives varied line breaks without a
/// Lorem-ipsum dependency.
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
    "baseline",
    "indent",
];

fn words(start: usize, count: usize) -> String {
    let mut s = String::new();
    for i in 0..count {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(WORDS[(start + i) % WORDS.len()]);
    }
    s
}

/// One paragraph split into plain / bold / italic runs so each carries multiple
/// style spans (exercising the per-run path, not one homogeneous run).
fn paragraph(seed: usize, n: usize) -> Block {
    let bold = (n / 4).max(1);
    let italic = (n / 4).max(1);
    let plain = n.saturating_sub(bold + italic).max(1);
    Block::Para(vec![
        Inline::Str(format!("{}. ", seed + 1)),
        Inline::Str(words(seed, plain)),
        Inline::Str(" ".into()),
        Inline::Strong(vec![Inline::Str(words(seed + plain, bold))]),
        Inline::Str(" ".into()),
        Inline::Emph(vec![Inline::Str(words(seed + plain + bold, italic))]),
    ])
}

/// Builds a [`Document`] of `paras` paragraphs (`words_per_para` words each) in a
/// single default-page section — the scale workload the doc benches sweep over.
pub fn build_doc(paras: usize, words_per_para: usize) -> Document {
    let blocks: Vec<Block> = (0..paras).map(|i| paragraph(i, words_per_para)).collect();
    let section = Section::with_layout_and_blocks(PageLayout::default(), blocks);
    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

/// Builds `chains` independent inheritance chains, each `depth` styles deep.
///
/// Only each chain's **root** sets `alignment`, so resolving a leaf walks the
/// full `depth` before finding the value (worst-case Inherited) — the §6 stressor
/// for "deep chains × many styles." Returns the catalog and the leaf ids to
/// resolve (one per chain). Total styles = `depth × chains`.
pub fn build_style_chains(depth: usize, chains: usize) -> (StyleCatalog, Vec<StyleId>) {
    let mut cat = StyleCatalog::new();
    let mut leaves = Vec::with_capacity(chains);
    for c in 0..chains {
        let mut parent: Option<StyleId> = None;
        for d in 0..depth {
            let id = StyleId::new(format!("c{c}s{d}"));
            let mut props = ParaProps::default();
            if d == 0 {
                props.alignment = Some(ParagraphAlignment::Center);
            }
            cat.paragraph_styles.insert(
                id.clone(),
                ParagraphStyle {
                    id: id.clone(),
                    display_name: None,
                    parent: parent.clone(),
                    linked_char_style: None,
                    para_props: props,
                    char_props: CharProps::default(),
                    next_style_id: None,
                    is_default: false,
                    is_custom: true,
                    extensions: ExtensionBag::default(),
                },
            );
            if d == depth - 1 {
                leaves.push(id.clone());
            }
            parent = Some(id);
        }
    }
    (cat, leaves)
}

/// Scale-corpus tiers for the document benches: `(label, paragraphs)`. Small ≈ a
/// page or two, Medium ≈ tens of pages, Large ≈ hundreds (Spec 06 §8).
pub const DOC_TIERS: &[(&str, usize)] = &[("small", 10), ("medium", 60), ("large", 250)];

/// Words per paragraph across the harness (a typical body paragraph).
pub const WORDS_PER_PARA: usize = 60;

/// Chain depths swept by the style-resolution bench (1 → deep).
pub const STYLE_DEPTHS: &[usize] = &[1, 4, 16, 64];

/// Chain counts swept by the style-resolution bench (few → pathological).
pub const STYLE_CHAINS: &[usize] = &[1, 100, 1000];

/// Prints a bench section header to stderr.
pub fn header(title: &str) {
    eprintln!("\n{title}\n  (portable allocation metrics — hardware-independent, Spec 06 D1)");
}

/// Prints one aligned metric row: label + cumulative bytes/allocs + live peak.
pub fn report_row(label: &str, s: AllocStats) {
    eprintln!(
        "  {label:<26} bytes={:>12} allocs={:>9} peak_bytes={:>12}",
        s.total_bytes, s.total_blocks, s.max_bytes,
    );
}
