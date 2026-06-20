// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Correctness gate for incremental paginated relayout.
//!
//! The driver may only return `Some` when its output is identical to a full
//! layout. These tests build a multi-page document, apply a battery of edits at
//! the start/middle/end, and assert that — whenever the incremental path fires —
//! its pages equal `layout_paginated_full`'s pages. Equality is compared on the
//! structural Debug of every page's content, the same exactness the paragraph
//! cache relies on. Sequential edits are also exercised so a stale checkpoint
//! (wrong reuse on the *next* edit) is caught too.

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::catalog::StyleCatalog;

use crate::{FontResources, LayoutOptions, PaginatedLayout, layout_paginated_full};

fn opts() -> LayoutOptions {
    LayoutOptions {
        preserve_for_editing: true,
    }
}

fn para(text: &str) -> Block {
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str(text.to_string())],
        attr: Default::default(),
    })
}

/// A multi-paragraph, multi-page single-section document (no footnotes).
fn doc_with(paragraphs: Vec<Block>) -> Document {
    let mut doc = Document::new_blank();
    doc.sections = vec![Section {
        layout: PageLayout::default(),
        blocks: paragraphs,
        extensions: Default::default(),
    }];
    doc.styles = StyleCatalog::default();
    doc
}

fn base_doc() -> Document {
    // Enough paragraphs to span several pages (kept modest: the property check
    // Debug-formats every page, which is the test's dominant cost).
    let mut blocks = Vec::new();
    for i in 0..48 {
        blocks.push(para(&format!(
            "Paragraph number {i} with enough words to occupy a reasonable \
             fraction of a line and exercise wrapping across the page width."
        )));
    }
    doc_with(blocks)
}

/// Replaces the text of block `idx` (assumed a single-`Str` paragraph).
fn edit_block(doc: &Document, idx: usize, text: &str) -> Document {
    let mut d = doc.clone();
    d.sections[0].blocks[idx] = para(text);
    d
}

/// Compares two paginated layouts for visual equality (page count + per-page
/// content/ header/footer Debug).
fn pages_eq(a: &PaginatedLayout, b: &PaginatedLayout) -> bool {
    if a.pages.len() != b.pages.len() {
        return false;
    }
    a.pages.iter().zip(b.pages.iter()).all(|(x, y)| {
        format!("{:?}", x.content_items) == format!("{:?}", y.content_items)
            && format!("{:?}", x.header_items) == format!("{:?}", y.header_items)
            && format!("{:?}", x.footer_items) == format!("{:?}", y.footer_items)
            && x.page_number == y.page_number
    })
}

/// Runs one incremental edit and asserts it equals a full layout (when the fast
/// path fires). Returns `(layout, reuse, fired)` where `fired` is whether the
/// incremental path produced the result (vs. falling back to full).
fn check_edit(
    fonts: &mut FontResources,
    prev_doc: &Document,
    prev: &(PaginatedLayout, crate::PaginatedReuse),
    new_doc: &Document,
    label: &str,
) -> (PaginatedLayout, crate::PaginatedReuse, bool) {
    let (full, full_reuse) = layout_paginated_full(fonts, new_doc, 1.0, &opts());

    match crate::relayout_paginated_incremental(
        fonts,
        new_doc,
        prev_doc,
        &prev.0,
        &prev.1,
        1.0,
        &opts(),
    ) {
        Some((inc, inc_reuse)) => {
            assert!(
                pages_eq(&inc, &full),
                "{label}: incremental layout diverged from full layout \
                 (incremental {} pages, full {} pages)",
                inc.pages.len(),
                full.pages.len(),
            );
            (inc, inc_reuse, true)
        }
        // Falling back to full is always allowed; just carry the full result.
        None => (full, full_reuse, false),
    }
}

#[test]
fn same_height_edits_match_full_layout() {
    let mut fonts = FontResources::new();
    let doc = base_doc();
    let prev = layout_paginated_full(&mut fonts, &doc, 1.0, &opts());
    assert!(prev.0.pages.len() > 2, "fixture should span multiple pages");
    assert!(!prev.1.has_footnotes);
    assert!(!prev.1.checkpoints.is_empty());

    // Same-length replacements at start, middle, end — height-preserving, so the
    // incremental path should fire and match the full layout exactly.
    let mut fired = false;
    for idx in [0usize, 24, 47] {
        let edited = edit_block(
            &doc,
            idx,
            "Paragraph number X with enough words to occupy a reasonable fraction of a line and exercise wrapping across the page width.",
        );
        let (_, _, f) = check_edit(
            &mut fonts,
            &doc,
            &prev,
            &edited,
            &format!("same-height edit @ {idx}"),
        );
        fired |= f;
    }
    assert!(
        fired,
        "incremental fast path never fired on height-preserving edits — the test \
         would be vacuous"
    );
}

#[test]
fn height_changing_edits_match_full_layout() {
    let mut fonts = FontResources::new();
    let doc = base_doc();
    let prev = layout_paginated_full(&mut fonts, &doc, 1.0, &opts());

    // Grow a paragraph by many lines (changes page breaks for the whole tail)
    // and shrink one to a single word. Either the fast path matches full, or it
    // declines — both are asserted safe by check_edit.
    let big = "word ".repeat(200);
    for (idx, text) in [(10usize, big.as_str()), (25, "x"), (47, "tiny")] {
        let edited = edit_block(&doc, idx, text);
        let _ = check_edit(
            &mut fonts,
            &doc,
            &prev,
            &edited,
            &format!("height-change edit @ {idx}"),
        );
    }
}

#[test]
fn sequential_edits_keep_matching() {
    let mut fonts = FontResources::new();
    let doc = base_doc();
    let mut cur_doc = doc.clone();
    let mut cur = layout_paginated_full(&mut fonts, &cur_doc, 1.0, &opts());

    // Apply a chain of edits, threading the incremental reuse metadata forward.
    // A stale checkpoint would surface as a divergence on a later edit.
    for n in 0..6 {
        let idx = (n * 7) % 48;
        let edited = edit_block(
            &cur_doc,
            idx,
            &format!("Edited paragraph {n} occupying a line or so of text across the page."),
        );
        let (layout, reuse, _) = check_edit(
            &mut fonts,
            &cur_doc,
            &cur,
            &edited,
            &format!("sequential edit {n} @ {idx}"),
        );
        cur = (layout, reuse);
        cur_doc = edited;
    }
}
