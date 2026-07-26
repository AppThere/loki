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

// ── Spec 09 E0 helpers (layout residency) ────────────────────────────────────

/// Counts the characters of display text in a document.
///
/// Built on `inline_plain_text`, which already flattens every inline variant.
/// A hand-rolled matcher over a subset of `Block`/`Inline` silently returned
/// zero for every real document in the corpus, because they use `StyledPara`
/// and `Heading` where [`build_doc`] uses `Para` — the quiet-wrong-answer shape
/// Spec 09 L9-009 now forbids.
pub fn char_count(doc: &Document) -> usize {
    use loki_doc_model::content::toc::inline_plain_text;

    fn block_chars(b: &Block) -> usize {
        match b {
            Block::Para(i) | Block::Plain(i) | Block::Heading(_, _, i) => {
                inline_plain_text(i).chars().count()
            }
            Block::StyledPara(p) => inline_plain_text(&p.inlines).chars().count(),
            Block::BlockQuote(inner) => inner.iter().map(block_chars).sum(),
            Block::OrderedList(_, items) | Block::BulletList(items) => items
                .iter()
                .flat_map(|blocks| blocks.iter())
                .map(block_chars)
                .sum(),
            Block::Table(t) => t
                .head
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
                .sum(),
            _ => 0,
        }
    }
    doc.sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .map(block_chars)
        .sum()
}

/// Conformance-corpus documents, as `(label, path relative to `loki-bench/`)`.
///
/// Read by path rather than by depending on `appthere-conformance`, so no crate
/// edge is added for the dependency-direction gate to weigh. Note the corpus is
/// **six** documents — the ~143 `TC-*` entries elsewhere in that crate are a
/// planned case catalog, not fixtures on disk.
pub const CORPUS: &[(&str, &str)] = &[
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
pub fn load_corpus_doc(rel: &str) -> Option<Document> {
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

/// Repeats a document's blocks `times` over, holding its formatting profile
/// constant while scaling size — the way to vary size independently of
/// formatting density (Spec 09 §4.1).
pub fn repeat_doc(doc: &Document, times: usize) -> Document {
    let mut out = doc.clone();
    for section in &mut out.sections {
        let original = section.blocks.clone();
        for _ in 1..times {
            section.blocks.extend(original.iter().cloned());
        }
    }
    out
}

/// Least-squares fit of `rate = intercept + slope · x`, returning
/// `(intercept, slope)`.
///
/// Used by the duplication sweep: with `x = unique/total` (i.e. `1/n` for an
/// n-fold repeated document), the intercept is the **per-placement** cost that
/// every copy pays and the slope is the **content-keyed** cost that
/// deduplicates. Returns `(0, 0)` for fewer than two distinct `x` values —
/// callers must not report a fit they did not get.
pub fn fit_line(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len() as f64;
    if points.len() < 2 {
        return (0.0, 0.0);
    }
    let sx: f64 = points.iter().map(|p| p.0).sum();
    let sy: f64 = points.iter().map(|p| p.1).sum();
    let sxx: f64 = points.iter().map(|p| p.0 * p.0).sum();
    let sxy: f64 = points.iter().map(|p| p.0 * p.1).sum();
    let denom = n * sxx - sx * sx;
    if denom.abs() < f64::EPSILON {
        return (0.0, 0.0);
    }
    let slope = (n * sxy - sx * sy) / denom;
    let intercept = (sy - slope * sx) / n;
    (intercept, slope)
}
