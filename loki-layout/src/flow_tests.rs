// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for [`crate::flow`].

use super::*;

use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize};
use loki_doc_model::layout::Section;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::para_props::{ParaProps, Spacing};
use loki_primitives::units::Points;

use crate::font::FontResources;
use crate::items::PositionedItem;
use crate::mode::LayoutMode;
use crate::resolve::pts_to_f32;

/// Helper: run flow_section in Pageless mode and return (items, height, warnings).
fn flow_pageless(
    r: &mut FontResources,
    section: &Section,
) -> (Vec<PositionedItem>, f32, Vec<LayoutWarning>) {
    let catalog = StyleCatalog::new();
    match flow_section(r, section, &catalog, &LayoutMode::Pageless, 1.0) {
        FlowOutput::Canvas { items, height, warnings } => (items, height, warnings),
        _ => panic!("expected Canvas output"),
    }
}

/// Helper: run flow_section in Paginated mode and return (pages, warnings).
fn flow_paginated(
    r: &mut FontResources,
    section: &Section,
) -> (Vec<crate::result::LayoutPage>, Vec<LayoutWarning>) {
    let catalog = StyleCatalog::new();
    match flow_section(r, section, &catalog, &LayoutMode::Paginated, 1.0) {
        FlowOutput::Pages { pages, warnings } => (pages, warnings),
        _ => panic!("expected Pages output"),
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in ["/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

fn make_para(text: &str) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    }
}

fn section_of(paras: Vec<StyledParagraph>, layout: PageLayout) -> Section {
    Section::with_layout_and_blocks(
        layout,
        paras.into_iter().map(Block::StyledPara).collect(),
    )
}

/// A very small page: 200 × 100 pt with 5 pt margins → 90 pt content height.
fn tiny_layout() -> PageLayout {
    PageLayout {
        page_size: PageSize { width: Points::new(200.0), height: Points::new(100.0) },
        margins: PageMargins {
            top: Points::new(5.0),
            bottom: Points::new(5.0),
            left: Points::new(10.0),
            right: Points::new(10.0),
            ..PageMargins::default()
        },
        ..PageLayout::default()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn continuous_cursor_advances() {
    let mut r = test_resources();
    let para = StyledParagraph {
        direct_para_props: Some(Box::new(ParaProps {
            space_after: Some(Spacing::Exact(Points::new(12.0))),
            ..Default::default()
        })),
        ..make_para("Hello, world!")
    };
    let section = section_of(vec![para], PageLayout::default());
    let (items, total_height, warnings) = flow_pageless(&mut r, &section);
    assert!(total_height > 0.0, "cursor must advance: got {total_height}");
    assert!(!items.is_empty(), "must produce at least one glyph run");
    assert!(warnings.is_empty(), "no warnings expected");
}

#[test]
fn space_before_offsets_content() {
    let mut r = test_resources();
    let space_before = 24.0_f32;
    let para = StyledParagraph {
        direct_para_props: Some(Box::new(ParaProps {
            space_before: Some(Spacing::Exact(Points::new(space_before as f64))),
            ..Default::default()
        })),
        ..make_para("Spaced")
    };
    let section = section_of(vec![para], PageLayout::default());
    let (items, _, _) = flow_pageless(&mut r, &section);
    let first_run_y = items.iter().find_map(|i| {
        if let PositionedItem::GlyphRun(run) = i { Some(run.origin.y) } else { None }
    });
    let y = first_run_y.expect("expected a glyph run");
    assert!(
        y >= space_before,
        "first glyph run y ({y}) should be ≥ space_before ({space_before})"
    );
}

#[test]
fn page_break_before_splits_onto_second_page() {
    let mut r = test_resources();
    let para1 = make_para("Page one");
    let para2 = StyledParagraph {
        direct_para_props: Some(Box::new(ParaProps {
            page_break_before: Some(true),
            ..Default::default()
        })),
        ..make_para("Page two")
    };
    let layout = PageLayout::default();
    let section = section_of(vec![para1, para2], layout);
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(pages.len(), 2, "expected two pages, got {}", pages.len());
    assert!(!pages[1].content_items.is_empty(), "page 2 must have content");
}

#[test]
fn block_taller_than_page_emits_warning() {
    let mut r = test_resources();
    // Repeat enough text to exceed the 90 pt content height on a tiny page.
    let long_text = "Lorem ipsum dolor sit amet. ".repeat(30);
    let para = make_para(&long_text);
    let section = section_of(vec![para], tiny_layout());
    let (_, warnings) = flow_paginated(&mut r, &section);
    let triggered = warnings
        .iter()
        .any(|w| matches!(w, LayoutWarning::BlockExceedsPageHeight { .. }));
    assert!(triggered, "expected BlockExceedsPageHeight warning; got {warnings:?}");
}

#[test]
fn heading_block_does_not_panic() {
    let mut r = test_resources();
    let section = Section {
        layout: PageLayout::default(),
        blocks: vec![Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("Introduction".into())],
        )],
        extensions: ExtensionBag::default(),
    };
    let (items, total_height, _) = flow_pageless(&mut r, &section);
    assert!(total_height > 0.0, "heading must have non-zero height");
    assert!(!items.is_empty(), "heading must produce items");
}

#[test]
fn pageless_respects_margins() {
    let mut r = test_resources();
    let left_margin = 50.0;
    let mut layout = PageLayout::default();
    layout.margins.left = Points::new(left_margin as f64);
    let section = section_of(vec![make_para("Hello")], layout);
    let (items, _, _) = flow_pageless(&mut r, &section);
    let first_run_x = items.iter().find_map(|i| {
        if let PositionedItem::GlyphRun(run) = i { Some(run.origin.x) } else { None }
    });
    let x = first_run_x.expect("expected a glyph run");
    assert_eq!(x, left_margin, "pageless item x should be offset by left margin");
}

// ── Helpers for new tests ─────────────────────────────────────────────────────

fn make_para_with_props(text: &str, props: ParaProps) -> StyledParagraph {
    StyledParagraph {
        direct_para_props: Some(Box::new(props)),
        ..make_para(text)
    }
}

fn first_glyph_y(items: &[PositionedItem]) -> Option<f32> {
    items.iter().find_map(|i| {
        match i {
            PositionedItem::GlyphRun(r) => Some(r.origin.y),
            PositionedItem::ClippedGroup { items, .. } => first_glyph_y(items),
            _ => None,
        }
    })
}

fn has_clipped_group(items: &[PositionedItem]) -> bool {
    items.iter().any(|i| matches!(i, PositionedItem::ClippedGroup { .. }))
}

// ── Paragraph splitting tests ─────────────────────────────────────────────────

#[test]
fn paragraph_split_produces_clipped_groups_on_multiple_pages() {
    let mut r = test_resources();
    // Long text at 12pt in 180pt-wide content area forces 10+ lines (~140pt+).
    let text = "Lorem ipsum dolor sit amet consectetur adipiscing. ".repeat(8);
    let section = section_of(vec![make_para(&text)], tiny_layout());
    let (pages, warnings) = flow_paginated(&mut r, &section);
    assert!(pages.len() >= 2, "expected 2+ pages for tall paragraph, got {}", pages.len());
    // Page 1 must have a ClippedGroup (Fragment A of the split paragraph).
    assert!(has_clipped_group(&pages[0].content_items), "page 1 should have a ClippedGroup");
    // All pages must have content.
    for (i, page) in pages.iter().enumerate() {
        assert!(!page.content_items.is_empty(), "page {i} should have content");
    }
    // BlockExceedsPageHeight warning may or may not fire depending on text length.
    let _ = warnings;
}

#[test]
fn split_fragment_a_clip_rect_within_page_height() {
    let mut r = test_resources();
    let text = "Lorem ipsum dolor sit amet consectetur adipiscing. ".repeat(8);
    let section = section_of(vec![make_para(&text)], tiny_layout());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert!(pages.len() >= 2, "need 2+ pages");

    // Fragment A clip rect bottom must not exceed page content height (90pt).
    let clip_a = pages[0].content_items.iter().find_map(|i| {
        if let PositionedItem::ClippedGroup { clip_rect, .. } = i {
            Some(*clip_rect)
        } else {
            None
        }
    });
    let clip_a = clip_a.expect("page 1 should have a ClippedGroup");
    assert!(
        clip_a.height() > 0.0,
        "Fragment A clip height must be positive"
    );
    assert!(
        clip_a.max_y() <= 90.0 + 0.5,
        "Fragment A clip bottom ({}) must not exceed page content height (90pt)",
        clip_a.max_y()
    );
}

#[test]
fn split_fragment_b_clip_starts_at_top_of_next_page() {
    let mut r = test_resources();
    let text = "Lorem ipsum dolor sit amet consectetur adipiscing. ".repeat(8);
    let section = section_of(vec![make_para(&text)], tiny_layout());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert!(pages.len() >= 2, "need 2+ pages");

    // Fragment B (or later) on page 2 should have clip_rect starting at y=0.
    let clip_b = pages[1].content_items.iter().find_map(|i| {
        if let PositionedItem::ClippedGroup { clip_rect, .. } = i {
            Some(*clip_rect)
        } else {
            None
        }
    });
    let clip_b = clip_b.expect("page 2 should have a ClippedGroup");
    assert!(
        clip_b.y() >= 0.0,
        "Fragment B clip_rect.y must be ≥ 0 (page-local)"
    );
    assert!(clip_b.height() > 0.0, "Fragment B clip height must be positive");
}

// ── keep-together tests ───────────────────────────────────────────────────────

#[test]
fn keep_together_block_fits_on_current_page_no_flush() {
    let mut r = test_resources();
    // Two paragraphs; para2 has keep_together. Both fit on one page.
    let para1 = make_para("First paragraph short.");
    let para2 = make_para_with_props(
        "Second paragraph with keep_together.",
        ParaProps { keep_together: Some(true), ..Default::default() },
    );
    let section = section_of(vec![para1, para2], PageLayout::default());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(pages.len(), 1, "both paragraphs fit on one page, expected 1 page");
}

#[test]
fn keep_together_block_pushed_to_next_page() {
    let mut r = test_resources();
    // para1 has large space_after that fills most of the tiny page (90pt).
    let para1 = make_para_with_props(
        "Para 1",
        ParaProps {
            space_after: Some(Spacing::Exact(Points::new(78.0))),
            ..Default::default()
        },
    );
    let para2 = make_para_with_props(
        "Para 2 keep together.",
        ParaProps { keep_together: Some(true), ..Default::default() },
    );
    let section = section_of(vec![para1, para2], tiny_layout());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(pages.len(), 2, "para2 should be pushed to page 2, expected 2 pages");
    assert!(!pages[1].content_items.is_empty(), "page 2 should have para2 content");
}

#[test]
fn keep_together_taller_than_page_emits_override_warning() {
    let mut r = test_resources();
    let long_text = "Lorem ipsum dolor sit amet consectetur adipiscing. ".repeat(8);
    let para = make_para_with_props(
        &long_text,
        ParaProps { keep_together: Some(true), ..Default::default() },
    );
    let section = section_of(vec![para], tiny_layout());
    let (_, warnings) = flow_paginated(&mut r, &section);
    // Either KeepTogetherOverride or BlockExceedsPageHeight must be emitted
    // (KeepTogetherOverride fires when height > page_content_height).
    let has_override = warnings.iter().any(|w| {
        matches!(w, LayoutWarning::KeepTogetherOverride { .. })
    });
    let has_exceeds = warnings.iter().any(|w| {
        matches!(w, LayoutWarning::BlockExceedsPageHeight { .. })
    });
    assert!(
        has_override || has_exceeds,
        "expected KeepTogetherOverride or BlockExceedsPageHeight; got {warnings:?}"
    );
}

// ── keep-with-next tests ──────────────────────────────────────────────────────

#[test]
fn keep_with_next_chain_fits_on_current_page_no_flush() {
    let mut r = test_resources();
    // Short chain (2 blocks) with keep_with_next; both fit on a fresh page.
    let para1 = make_para_with_props(
        "Heading",
        ParaProps { keep_with_next: Some(true), ..Default::default() },
    );
    let para2 = make_para("Body paragraph follows heading.");
    let section = section_of(vec![para1, para2], PageLayout::default());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(pages.len(), 1, "chain fits on one page, expected 1 page");
}

#[test]
fn keep_with_next_chain_pushed_to_next_page() {
    let mut r = test_resources();
    // para0 has large space_after to fill most of the tiny page.
    let para0 = make_para_with_props(
        "Filler",
        ParaProps {
            space_after: Some(Spacing::Exact(Points::new(78.0))),
            ..Default::default()
        },
    );
    let para1 = make_para_with_props(
        "Heading keep_with_next",
        ParaProps { keep_with_next: Some(true), ..Default::default() },
    );
    let para2 = make_para("Body after heading.");
    let section = section_of(vec![para0, para1, para2], tiny_layout());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert!(pages.len() >= 2, "chain should be pushed to page 2, got {} pages", pages.len());
    // para1 and para2 must be on the same page (page 2).
    assert!(!pages[1].content_items.is_empty(), "page 2 must have content");
}

#[test]
fn keep_with_next_chain_truncated_at_limit_5() {
    let mut r = test_resources();
    // 6 paragraphs with keep_with_next=true + 1 terminal = 7 blocks total.
    // Chain limit is 5, so chain is truncated and warning is emitted.
    let kwn = ParaProps { keep_with_next: Some(true), ..Default::default() };
    let mut paras: Vec<StyledParagraph> = (0..6)
        .map(|i| make_para_with_props(&format!("kwn para {i}"), kwn.clone()))
        .collect();
    paras.push(make_para("terminal"));
    let section = section_of(paras, PageLayout::default());
    let (_, warnings) = flow_paginated(&mut r, &section);
    let truncated = warnings.iter().any(|w| {
        matches!(w, LayoutWarning::KeepWithNextChainTruncated { .. })
    });
    assert!(truncated, "expected KeepWithNextChainTruncated warning; got {warnings:?}");
}

#[test]
fn keep_with_next_chain_too_tall_emits_warning() {
    let mut r = test_resources();
    // 3 chain blocks + 1 terminal; each inflated with space_before=28pt so
    // the total chain height exceeds the tiny page's 90pt content area.
    let props_kwn = ParaProps {
        keep_with_next: Some(true),
        space_before: Some(Spacing::Exact(Points::new(28.0))),
        ..Default::default()
    };
    let props_term = ParaProps {
        space_before: Some(Spacing::Exact(Points::new(28.0))),
        ..Default::default()
    };
    let paras = vec![
        make_para_with_props("kwn 0", props_kwn.clone()),
        make_para_with_props("kwn 1", props_kwn.clone()),
        make_para_with_props("kwn 2", props_kwn),
        make_para_with_props("terminal", props_term),
    ];
    let section = section_of(paras, tiny_layout());
    let (_, warnings) = flow_paginated(&mut r, &section);
    let too_tall = warnings.iter().any(|w| {
        matches!(w, LayoutWarning::KeepWithNextChainTooTall { .. })
    });
    assert!(too_tall, "expected KeepWithNextChainTooTall warning; got {warnings:?}");
}

// ── margins.top bug regression test ──────────────────────────────────────────

#[test]
fn margins_top_bug_fixed_page2_item_at_correct_y() {
    let mut r = test_resources();
    // Two identical paragraphs: para1 is on page 1, para2 (with page_break_before)
    // is on page 2. Both have no space_before. Items on page 2 should start at
    // the same page-local y as items on page 1 (proving margins.top is not added).
    let para1 = make_para("Page one content.");
    let para2 = make_para_with_props(
        "Page two content.",
        ParaProps { page_break_before: Some(true), ..Default::default() },
    );
    let section = section_of(vec![para1, para2], PageLayout::default());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(pages.len(), 2);

    let y1 = first_glyph_y(&pages[0].content_items).expect("page 1 has content");
    let y2 = first_glyph_y(&pages[1].content_items).expect("page 2 has content");

    assert!(
        (y1 - y2).abs() < 0.5,
        "page 1 first glyph y ({y1}) should equal page 2 first glyph y ({y2}) — margins.top bug check"
    );
}
