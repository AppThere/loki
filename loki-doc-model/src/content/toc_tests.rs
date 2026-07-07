// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the pure table-of-contents builders.

use super::*;
use crate::content::attr::NodeAttr;
use crate::layout::page::PageLayout;

fn heading(level: u8, text: &str) -> Block {
    Block::Heading(level, NodeAttr::default(), vec![Inline::Str(text.into())])
}

fn section_with(blocks: Vec<Block>) -> Section {
    Section::with_layout_and_blocks(PageLayout::default(), blocks)
}

#[test]
fn plain_text_flattens_nested_formatting() {
    let inlines = vec![
        Inline::Str("Chapter".into()),
        Inline::Space,
        Inline::Strong(vec![Inline::Str("One".into())]),
    ];
    assert_eq!(inline_plain_text(&inlines), "Chapter One");
}

#[test]
fn plain_text_drops_non_text_objects() {
    // A bookmark anchor carries no label text; it must not appear.
    let inlines = vec![
        Inline::Str("Title".into()),
        Inline::Bookmark(crate::content::inline::BookmarkKind::Start, "bm".into()),
    ];
    assert_eq!(inline_plain_text(&inlines), "Title");
}

#[test]
fn outline_collects_headings_in_order_within_depth() {
    let sections = vec![
        section_with(vec![
            heading(1, "Intro"),
            Block::Para(vec![Inline::Str("body".into())]),
            heading(2, "Background"),
        ]),
        section_with(vec![heading(1, "Method"), heading(4, "Too deep")]),
    ];
    let outline = heading_outline(&sections, DEFAULT_TOC_DEPTH);
    assert_eq!(
        outline,
        vec![
            (1, "Intro".to_string()),
            (2, "Background".to_string()),
            (1, "Method".to_string()),
        ],
        "level-4 heading is beyond the default depth of 3"
    );
}

#[test]
fn depth_bound_is_respected() {
    let sections = vec![section_with(vec![
        heading(1, "A"),
        heading(2, "B"),
        heading(3, "C"),
    ])];
    assert_eq!(heading_outline(&sections, 1), vec![(1, "A".to_string())]);
    assert_eq!(heading_outline(&sections, 2).len(), 2);
}

#[test]
fn build_toc_indents_entries_by_level() {
    let sections = vec![section_with(vec![heading(1, "Top"), heading(2, "Sub")])];
    let toc = build_toc(&sections, None, DEFAULT_TOC_DEPTH);
    assert_eq!(toc.body.len(), 2);
    // Level 1 → no indent; level 2 → one step.
    let indent = |b: &Block| match b {
        Block::StyledPara(p) => p
            .direct_para_props
            .as_ref()
            .and_then(|pp| pp.indent_start)
            .map(|p| p.value()),
        _ => None,
    };
    assert_eq!(indent(&toc.body[0]), Some(0.0));
    assert_eq!(indent(&toc.body[1]), Some(18.0));
}

#[test]
fn build_toc_prepends_a_bold_title_that_is_not_an_outline_heading() {
    let sections = vec![section_with(vec![heading(1, "Top")])];
    let toc = build_toc(&sections, Some("Contents"), DEFAULT_TOC_DEPTH);
    assert_eq!(toc.body.len(), 2, "title paragraph + one entry");
    // The title is a bold paragraph, NOT a Heading (so a refresh won't list it).
    match &toc.body[0] {
        Block::StyledPara(p) => {
            assert!(matches!(p.inlines.first(), Some(Inline::Strong(_))));
        }
        other => panic!("expected a styled title paragraph, got {other:?}"),
    }
    // Re-deriving the outline from a body-with-title finds only the real heading.
    assert_eq!(heading_outline(&sections, DEFAULT_TOC_DEPTH).len(), 1);
}

#[test]
fn empty_document_yields_an_empty_toc() {
    let toc = build_toc(&[], None, DEFAULT_TOC_DEPTH);
    assert!(toc.body.is_empty());
}
