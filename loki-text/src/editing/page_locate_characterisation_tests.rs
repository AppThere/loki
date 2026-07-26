// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Characterisation of [`super::recompute_page_index`] against a **real** laid-out
//! document (Spec 09 R9-18).
//!
//! These are not specification tests. They assert what the function *does* on
//! geometry the flow engine actually produces, so that a future replacement —
//! S9-3 wants to make this a block→page index lookup — can be checked against
//! observed behaviour rather than against what the code appears to intend. A
//! rewrite built by reading the source reproduces the intent; if the intent and
//! the behaviour differ, that rewrite silently changes where the caret lands.
//!
//! The sibling `page_locate_tests.rs` builds `PaginatedLayout` values by hand,
//! which is right for exercising the decision rules in isolation and wrong for
//! this purpose: hand-built geometry is the geometry the author expected. These
//! lay out a document and use whatever comes out.
//!
//! # What prompted this
//!
//! A timing bench (`benches/page_locate_latency.rs`) found the call's cost flat
//! in the caret's page — the same at page 0 and page 444 — and a guaranteed full
//! scan costing the same as any hit. The reading offered at the time was that the
//! `visible` early exit never fires and the answer always comes from
//! `first_holder`, which was written into Spec 09 as R9-18.
//!
//! **These tests refute that.** `visible` does fire on real flow geometry: the
//! last byte of a split paragraph resolves to a later page, which only the band
//! check can produce. R9-18 is retracted, and the flat timing is left unexplained
//! rather than re-explained — swapping one inference for another is how the
//! first one got written down.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_layout::{
    DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout, layout_document,
};

use super::recompute_page_index;
use crate::editing::cursor::DocumentPosition;

/// Lays out `blocks` the way the app does, returning the real paginated result.
fn lay_out(blocks: Vec<Block>) -> PaginatedLayout {
    let section = Section::with_layout_and_blocks(PageLayout::default(), blocks);
    let mut doc = Document::new();
    doc.sections = vec![section];
    let mut resources = FontResources::new();
    match layout_document(
        &mut resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions {
            preserve_for_editing: true,
            spell: None,
            ..Default::default()
        },
    ) {
        DocumentLayout::Paginated(p) => p,
        other => panic!("paginated mode returned {other:?}"),
    }
}

fn para(text: String) -> Block {
    Block::Para(vec![Inline::Str(text)])
}

/// Words enough to fill roughly `lines` lines of a default-width page.
fn filler(seed: usize, words: usize) -> String {
    const POOL: &[&str] = &[
        "document",
        "layout",
        "paragraph",
        "cursor",
        "render",
        "measure",
        "baseline",
        "column",
    ];
    let mut s = String::new();
    for i in 0..words {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(POOL[(seed + i) % POOL.len()]);
    }
    s
}

/// Every page index at which `block` appears in the editing index.
fn pages_holding(layout: &PaginatedLayout, block: usize) -> Vec<usize> {
    layout
        .pages
        .iter()
        .enumerate()
        .filter(|(_, page)| {
            page.editing_data.as_ref().is_some_and(|ed| {
                ed.paragraphs
                    .iter()
                    .any(|p| p.block_index == block && p.path.is_empty())
            })
        })
        .map(|(i, _)| i)
        .collect()
}

/// **The R9-18 discriminator.**
///
/// For a paragraph on exactly one page, `visible` and `first_holder` give the
/// same answer, so no return value can tell them apart — which is why the bench
/// could only infer. A paragraph split across a page break separates them:
/// `first_holder` is the earlier page for *every* byte offset, while `visible` is
/// whichever page actually renders the caret's line.
///
/// So a late byte offset resolving to the later page means `visible` fired —
/// and it does, which is what retracted R9-18.
#[test]
fn a_split_paragraph_resolves_late_bytes_to_the_later_page() {
    // One paragraph long enough that the flow engine must break it across pages,
    // preceded by filler so the break lands mid-paragraph rather than at its top.
    let long_text = filler(0, 4_000);
    let blocks = vec![para(filler(3, 200)), para(long_text.clone())];
    let layout = lay_out(blocks);

    let holders = pages_holding(&layout, 1);
    assert!(
        holders.len() >= 2,
        "test premise: block 1 must span a page break, but it is on pages {holders:?} \
         of {} — increase the filler length if pagination changed",
        layout.pages.len()
    );
    let (first_page, later_page) = (holders[0], holders[1]);

    // Byte 0 is on the first fragment.
    let at_start = recompute_page_index(&layout, &DocumentPosition::top_level(0, 1, 0));
    assert_eq!(
        at_start.page_index, first_page,
        "the paragraph's first byte should resolve to its first page"
    );

    // The last byte is rendered on the last page holding the paragraph.
    let last_offset = long_text.len();
    let at_end = recompute_page_index(&layout, &DocumentPosition::top_level(0, 1, last_offset));

    // The characterisation, stated as an observation rather than a rule.
    //
    // Passing means the answer is NOT `first_holder`'s, so the `visible` band
    // check fired — which is what retracted R9-18. A replacement must reproduce
    // this: resolving a split paragraph's late bytes to its first page would put
    // the caret on the wrong page, and no existing test outside this file would
    // notice, because the hand-built ones supply their own geometry.
    assert_ne!(
        at_end.page_index, first_page,
        "the last byte of a split paragraph resolved to its FIRST page, which is \
         `first_holder`'s answer — the `visible` band check no longer fires on \
         real flow geometry. Either pagination changed under this test or a \
         replacement has reintroduced R9-18's behaviour for real."
    );
    assert!(
        at_end.page_index >= later_page,
        "the last byte resolved to page {} but the paragraph continues to page \
         {later_page} or beyond ({holders:?})",
        at_end.page_index
    );
}

/// A paragraph living on exactly one page resolves to that page, from a stale
/// index, at both ends of its text.
///
/// This is the case every keystroke hits, and the one the timing bench probed.
/// Both candidate branches agree here — which is precisely why it could not
/// discriminate — so it is recorded as behaviour a replacement must preserve
/// rather than as evidence about the branch.
#[test]
fn a_single_page_paragraph_resolves_from_a_stale_index() {
    let blocks: Vec<Block> = (0..400).map(|i| para(filler(i, 60))).collect();
    let layout = lay_out(blocks);
    assert!(
        layout.pages.len() > 3,
        "test premise: several pages, got {}",
        layout.pages.len()
    );

    // Pick a block that sits on exactly one page, well into the document.
    let (block, page) = (0..400)
        .filter_map(|b| {
            let h = pages_holding(&layout, b);
            (h.len() == 1 && h[0] >= 2).then(|| (b, h[0]))
        })
        .next()
        .expect("some paragraph sits on exactly one page past page 1");

    for offset in [0usize, 10] {
        let out = recompute_page_index(&layout, &DocumentPosition::top_level(0, block, offset));
        assert_eq!(
            out.page_index, page,
            "block {block} at byte {offset} should resolve to its only page {page}"
        );
    }
}

/// A block index that exists nowhere leaves the position untouched.
///
/// The `None, None, None` arm. Worth pinning because a block→page index will
/// return "not found" through a different route, and the current contract is to
/// return the input unchanged rather than to clamp or to zero.
#[test]
fn an_absent_block_leaves_the_position_unchanged() {
    let blocks: Vec<Block> = (0..40).map(|i| para(filler(i, 60))).collect();
    let layout = lay_out(blocks);

    let pos = DocumentPosition::top_level(2, 9_999, 0);
    let out = recompute_page_index(&layout, &pos);
    assert_eq!(
        out.page_index, 2,
        "an unknown block must leave page_index alone, not reset it"
    );
    assert_eq!(out.paragraph_index, pos.paragraph_index);
    assert_eq!(out.byte_offset, pos.byte_offset);
}
