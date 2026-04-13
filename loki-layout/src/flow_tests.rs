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
    let catalog = StyleCatalog::new();
    let para = StyledParagraph {
        direct_para_props: Some(Box::new(ParaProps {
            space_after: Some(Spacing::Exact(Points::new(12.0))),
            ..Default::default()
        })),
        ..make_para("Hello, world!")
    };
    let section = section_of(vec![para], PageLayout::default());
    let (items, total_height, warnings) = flow_section(
        &mut r, &section, &catalog, &LayoutMode::Pageless, 1.0,
    );
    assert!(total_height > 0.0, "cursor must advance: got {total_height}");
    assert!(!items.is_empty(), "must produce at least one glyph run");
    assert!(warnings.is_empty(), "no warnings expected");
}

#[test]
fn space_before_offsets_content() {
    let mut r = test_resources();
    let catalog = StyleCatalog::new();
    let space_before = 24.0_f32;
    let para = StyledParagraph {
        direct_para_props: Some(Box::new(ParaProps {
            space_before: Some(Spacing::Exact(Points::new(space_before as f64))),
            ..Default::default()
        })),
        ..make_para("Spaced")
    };
    let section = section_of(vec![para], PageLayout::default());
    let (items, _, _) = flow_section(
        &mut r, &section, &catalog, &LayoutMode::Pageless, 1.0,
    );
    // The first glyph run baseline should be at y ≥ space_before.
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
    let catalog = StyleCatalog::new();
    let para1 = make_para("Page one");
    let para2 = StyledParagraph {
        direct_para_props: Some(Box::new(ParaProps {
            page_break_before: Some(true),
            ..Default::default()
        })),
        ..make_para("Page two")
    };
    let layout = PageLayout::default();
    let page_h = pts_to_f32(layout.page_size.height);
    let section = section_of(vec![para1, para2], layout);
    let (items, total_height, _) = flow_section(
        &mut r, &section, &catalog, &LayoutMode::Paginated, 1.0,
    );
    assert_eq!(total_height, page_h * 2.0, "two pages expected");
    // Items on page 2 have y > page_h (they are offset by page_h + margins.top).
    let has_page2_item = items.iter().any(|item| {
        matches!(item, PositionedItem::GlyphRun(r) if r.origin.y > page_h)
    });
    assert!(has_page2_item, "at least one item should be on page 2 (y > {page_h})");
}

#[test]
fn block_taller_than_page_emits_warning() {
    let mut r = test_resources();
    let catalog = StyleCatalog::new();
    // Repeat enough text to exceed the 90 pt content height on a tiny page.
    let long_text = "Lorem ipsum dolor sit amet. ".repeat(30);
    let para = make_para(&long_text);
    let section = section_of(vec![para], tiny_layout());
    let (_, _, warnings) = flow_section(
        &mut r, &section, &catalog, &LayoutMode::Paginated, 1.0,
    );
    let triggered = warnings
        .iter()
        .any(|w| matches!(w, LayoutWarning::BlockExceedsPageHeight { .. }));
    assert!(triggered, "expected BlockExceedsPageHeight warning; got {warnings:?}");
}

#[test]
fn heading_block_does_not_panic() {
    let mut r = test_resources();
    let catalog = StyleCatalog::new();
    let section = Section {
        layout: PageLayout::default(),
        blocks: vec![Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("Introduction".into())],
        )],
        extensions: ExtensionBag::default(),
    };
    let (items, total_height, _) = flow_section(
        &mut r, &section, &catalog, &LayoutMode::Pageless, 1.0,
    );
    assert!(total_height > 0.0, "heading must have non-zero height");
    assert!(!items.is_empty(), "heading must produce items");
}

#[test]
fn pageless_respects_margins() {
    let mut r = test_resources();
    let catalog = StyleCatalog::new();
    let left_margin = 50.0;
    let mut layout = PageLayout::default();
    layout.margins.left = Points::new(left_margin as f64);
    
    let section = section_of(vec![make_para("Hello")], layout);
    let (items, _, _) = flow_section(
        &mut r, &section, &catalog, &LayoutMode::Pageless, 1.0,
    );
    
    let first_run_x = items.iter().find_map(|i| {
        if let PositionedItem::GlyphRun(run) = i { Some(run.origin.x) } else { None }
    });
    let x = first_run_x.expect("expected a glyph run");
    assert_eq!(x, left_margin, "pageless item x should be offset by left margin");
}
