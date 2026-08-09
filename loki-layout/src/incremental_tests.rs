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
        spell: None,
        ..Default::default()
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
        page_style: None,
        layout: PageLayout::default(),
        start: Default::default(),
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

/// A three-section document, each section spanning multiple pages.
fn multi_section_doc() -> Document {
    let mut doc = Document::new_blank();
    let mut sections = Vec::new();
    for s in 0..3 {
        let mut blocks = Vec::new();
        for i in 0..30 {
            blocks.push(para(&format!(
                "Section {s} paragraph {i} with enough words to wrap across the \
                 page width and occupy a couple of lines of body content."
            )));
        }
        sections.push(Section {
            page_style: None,
            layout: PageLayout::default(),
            start: Default::default(),
            blocks,
            extensions: Default::default(),
        });
    }
    doc.sections = sections;
    doc.styles = StyleCatalog::default();
    doc
}

/// Inserts a new paragraph after block `idx` in `section` (an Enter-like split).
fn insert_block(doc: &Document, section: usize, idx: usize, text: &str) -> Document {
    let mut d = doc.clone();
    d.sections[section].blocks.insert(idx + 1, para(text));
    d
}

/// Deletes block `idx` in `section`.
fn delete_block(doc: &Document, section: usize, idx: usize) -> Document {
    let mut d = doc.clone();
    d.sections[section].blocks.remove(idx);
    d
}

/// Flips the first character of block `idx` in `section` — a length-preserving
/// (height-preserving) edit confined to that section.
fn flip_char(doc: &Document, section: usize, idx: usize) -> Document {
    let mut d = doc.clone();
    if let Block::StyledPara(p) = &mut d.sections[section].blocks[idx]
        && let Some(Inline::Str(t)) = p.inlines.first_mut()
        && !t.is_empty()
    {
        let mut chars: Vec<char> = t.chars().collect();
        chars[0] = if chars[0] == 'Z' { 'Y' } else { 'Z' };
        *t = chars.into_iter().collect();
    }
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
fn block_insert_delete_match_full_layout() {
    let mut fonts = FontResources::new();
    let doc = base_doc();
    let prev = layout_paginated_full(&mut fonts, &doc, 1.0, &opts());

    // Inserts (Enter-like) at start / middle / end. A single-section document is
    // its own last section, so these fire even when they add a page.
    let mut fired = false;
    for idx in [3usize, 24, 46] {
        let edited = insert_block(&doc, 0, idx, "Inserted short paragraph of text.");
        let (_, _, f) = check_edit(&mut fonts, &doc, &prev, &edited, &format!("insert @ {idx}"));
        fired |= f;
    }
    // Deletions (Backspace-at-boundary-like).
    for idx in [2usize, 30] {
        let edited = delete_block(&doc, 0, idx);
        let (_, _, f) = check_edit(&mut fonts, &doc, &prev, &edited, &format!("delete @ {idx}"));
        fired |= f;
    }
    assert!(fired, "incremental should fire on block insert/delete");
}

#[test]
fn multi_section_block_insert_delete_match_full() {
    let mut fonts = FontResources::new();
    let doc = multi_section_doc();
    let prev = layout_paginated_full(&mut fonts, &doc, 1.0, &opts());

    // Insert/delete in a middle section and the first section. Either the fast
    // path fires (page-neutral → later sections reused) or it declines; both are
    // asserted equal to a full layout by check_edit.
    let cases = [
        insert_block(&doc, 1, 15, "Inserted line in section one."),
        delete_block(&doc, 0, 10),
        insert_block(&doc, 2, 5, "Inserted line in the last section."),
    ];
    for (i, edited) in cases.iter().enumerate() {
        let _ = check_edit(&mut fonts, &doc, &prev, edited, &format!("ms ins/del {i}"));
    }
}

#[test]
fn multi_section_edits_match_full_layout() {
    let mut fonts = FontResources::new();
    let doc = multi_section_doc();
    let prev = layout_paginated_full(&mut fonts, &doc, 1.0, &opts());
    assert!(prev.0.pages.len() > 3, "fixture should span multiple pages");
    // Checkpoints must span all three sections.
    assert!(
        prev.1.checkpoints.iter().any(|c| c.section_index == 2),
        "checkpoints should be tagged with their section"
    );

    // A height-preserving edit in *each* section must fire and match full.
    let mut fired = false;
    for s in [0usize, 1, 2] {
        let edited = flip_char(&doc, s, 15);
        let (_, _, f) = check_edit(
            &mut fonts,
            &doc,
            &prev,
            &edited,
            &format!("multi-section edit @ s{s}"),
        );
        fired |= f;
    }
    assert!(
        fired,
        "incremental should fire on height-preserving multi-section edits"
    );

    // A height-*changing* edit in a non-last section must still match full
    // (the driver declines and check_edit falls back).
    let grown = {
        let mut d = doc.clone();
        d.sections[0].blocks[15] = para(&"word ".repeat(120));
        d
    };
    let _ = check_edit(&mut fonts, &doc, &prev, &grown, "multi-section grow @ s0");
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

/// Records how much of the incremental path the property tests above actually
/// reach — which is currently far less than their names suggest (Spec 08 R30).
///
/// `check_edit` asserts `incremental == full`, and that assertion is real. But a
/// resume can only begin at a checkpoint, and this fixture produces **one**, at
/// block 0. So every "incremental" result those tests compare was produced by
/// resuming from the very start of the document and re-flowing all of it. The
/// property they guard — that a resume from an *arbitrary* point reproduces a full
/// layout — has never been exercised.
///
/// That is the `preserve_for_editing: false` trap in a different costume: a test
/// that passes without running the thing it protects. It is recorded here as a
/// measured fact rather than a comment so that **T3.4 will fail this test**, which
/// is the intended signal — at that point the checkpoint count becomes the page
/// count, resume-from-anywhere starts happening for the first time, and the
/// property tests above become meaningful.
///
/// When T3.4 lands: update the expectation to the page count, and treat the
/// silent-splice hazard as *newly* covered — but verify that coverage red-first, by
/// introducing an off-by-one on the resume line index and confirming `check_edit`
/// catches it. If it does not, the top-ranked hazard has no net beneath it.
#[test]
fn the_property_tests_only_ever_resume_from_block_zero() {
    let mut fonts = FontResources::new();
    let doc = base_doc();
    let (layout, reuse) = layout_paginated_full(&mut fonts, &doc, 1.0, &opts());

    assert!(
        layout.pages.len() > 1,
        "fixture must span several pages for this to say anything",
    );
    assert_eq!(
        reuse.checkpoints.len(),
        1,
        "expected the pre-T3.4 state: one checkpoint per document. If this now \
         equals the page count ({}), T3.4 has landed — see this test's docs for \
         what to do next.",
        layout.pages.len(),
    );
    assert_eq!(
        reuse.checkpoints[0].block_index, 0,
        "the single checkpoint is at block 0, so every resume starts there",
    );
}

/// **The incremental path must agree with the full path about headers, too.**
///
/// `pages_eq` has always compared `header_items` and `footer_items`, but no
/// fixture in this file gave any section a header — so the comparison was
/// between two empty vectors and could not fail. An instrument that cannot
/// speak where the hazard is.
///
/// **What this does and does not establish.** It closes the empty-vs-empty
/// blind spot: headers now render on both sides, so a divergence in *which*
/// variant a page got is visible. It does **not** yet discriminate
/// `assign_headers_footers`'s `section_first_page` argument on this path —
/// per `the_property_tests_only_ever_resume_from_block_zero`, this fixture
/// yields one checkpoint per section at block 0, so the re-flowed middle always
/// *is* the whole section and `sc_start + 1` equals `pages.first()`. Passing the
/// value in is correct in advance of T3.4 rather than a fix for a live
/// divergence; when T3.4 lands and resumes start mid-section, this test gains
/// that discrimination and should be re-mutation-checked.
///
/// The PAGE field in the default header is what makes the restart observable:
/// without it `display_pn` is computed and never rendered.
#[test]
fn multi_section_header_variants_match_full_layout() {
    use loki_doc_model::content::field::types::{Field, FieldKind};
    use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};

    let page_number_para = || {
        Block::StyledPara(StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Field(
                Field::new(FieldKind::PageNumber).with_current_value("1"),
            )],
            attr: Default::default(),
        })
    };

    let mut fonts = FontResources::new();
    let mut doc = multi_section_doc();
    for (i, s) in doc.sections.iter_mut().enumerate() {
        s.layout.header_first = Some(HeaderFooter {
            kind: HeaderFooterKind::First,
            blocks: vec![para(&"F".repeat(i + 1))],
        });
        s.layout.header_even = Some(HeaderFooter {
            kind: HeaderFooterKind::Even,
            blocks: vec![para(&"E".repeat(i + 4))],
        });
        // Carries a PAGE field so the numbering restart below is rendered
        // rather than merely computed.
        s.layout.header = Some(HeaderFooter {
            kind: HeaderFooterKind::Default,
            blocks: vec![page_number_para()],
        });
        s.layout.page_number_start = Some(1);
    }

    let prev = layout_paginated_full(&mut fonts, &doc, 1.0, &opts());
    assert!(
        prev.0.pages.len() > 3,
        "fixture should span multiple pages, got {}",
        prev.0.pages.len()
    );
    // The fixture is only discriminating if the headers actually rendered —
    // an empty header on both sides is the blind spot this test exists to close.
    assert!(
        prev.0.pages.iter().any(|p| !p.header_items.is_empty()),
        "no page rendered a header — the comparison would be vacuous"
    );

    // One edit per section, at a different depth in each: `check_edit` runs a
    // full layout and Debug-formats every page, so the count is kept to what
    // distinguishes the sections rather than a sweep.
    let mut fired = false;
    for (s, idx) in [(0usize, 0usize), (1, 15), (2, 29)] {
        let edited = flip_char(&doc, s, idx);
        let (_, _, f) = check_edit(
            &mut fonts,
            &doc,
            &prev,
            &edited,
            &format!("header edit @ s{s} b{idx}"),
        );
        fired |= f;
    }
    assert!(
        fired,
        "incremental never fired — the comparison would be vacuous"
    );
}
