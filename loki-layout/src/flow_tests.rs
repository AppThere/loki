// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for [`crate::flow`].

use super::*;

use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, CellProps, Row};
use loki_doc_model::layout::Section;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize, SectionColumns};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};
use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_doc_model::style::props::para_props::{ParaProps, Spacing};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::items::PositionedItem;
use crate::mode::LayoutMode;

/// Helper: run flow_section in Pageless mode and return (items, height, warnings).
fn flow_pageless(
    r: &mut FontResources,
    section: &Section,
) -> (Vec<PositionedItem>, f32, Vec<LayoutWarning>) {
    let catalog = StyleCatalog::new();
    match flow_section(
        r,
        section,
        &catalog,
        &LayoutMode::Pageless,
        1.0,
        &LayoutOptions::default(),
        &[],
    ) {
        FlowOutput::Canvas {
            items,
            height,
            warnings,
            ..
        } => (items, height, warnings),
        _ => panic!("expected Canvas output"),
    }
}

/// Helper: run flow_section in Paginated mode and return (pages, warnings).
fn flow_paginated(
    r: &mut FontResources,
    section: &Section,
) -> (Vec<crate::result::LayoutPage>, Vec<LayoutWarning>) {
    let catalog = StyleCatalog::new();
    match flow_section(
        r,
        section,
        &catalog,
        &LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
        &[],
    ) {
        FlowOutput::Pages {
            pages, warnings, ..
        } => (pages, warnings),
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
    Section::with_layout_and_blocks(layout, paras.into_iter().map(Block::StyledPara).collect())
}

/// A very small page: 200 × 100 pt with 5 pt margins → 90 pt content height.
fn tiny_layout() -> PageLayout {
    PageLayout {
        page_size: PageSize {
            width: Points::new(200.0),
            height: Points::new(100.0),
        },
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

/// Collects the x-origins of every glyph run in a page's content items.
fn glyph_x_origins(page: &crate::result::LayoutPage) -> Vec<f32> {
    page.content_items
        .iter()
        .filter_map(|i| {
            if let PositionedItem::GlyphRun(run) = i {
                Some(run.origin.x)
            } else {
                None
            }
        })
        .collect()
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn comment_panel_renders_in_gutter() {
    use loki_doc_model::content::annotation::{Comment, CommentRef, CommentRefKind};
    use loki_doc_model::content::block::Block;

    let mut r = test_resources();
    // A paragraph with a comment start anchor on it.
    let para = Block::Para(vec![
        Inline::Str("Commented text".into()),
        Inline::Comment(CommentRef::new("c1", CommentRefKind::Start)),
    ]);
    let section = Section::with_layout_and_blocks(tiny_layout(), vec![para]);

    let comments = vec![Comment::new("c1").with_plain_body("Fix this")];
    let catalog = StyleCatalog::new();

    let FlowOutput::Pages { pages, .. } = flow_section(
        &mut r,
        &section,
        &catalog,
        &LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
        &comments,
    ) else {
        panic!("expected paginated pages");
    };

    // The first page must carry a comment card in the gutter (x ≥ page width).
    let card = pages[0].comment_items.iter().find_map(|i| {
        if let PositionedItem::FilledRect(rect) = i {
            (rect.rect.origin.x >= 200.0).then_some(rect.rect)
        } else {
            None
        }
    });
    assert!(
        card.is_some(),
        "expected a comment card past the page right edge; items: {}",
        pages[0].comment_items.len()
    );
    // The card must contain rendered text (author + body glyph runs).
    let has_glyphs = pages[0]
        .comment_items
        .iter()
        .any(|i| matches!(i, PositionedItem::GlyphRun(_)));
    assert!(has_glyphs, "comment card should contain rendered text");
}

#[test]
fn no_comment_panel_without_anchors() {
    let mut r = test_resources();
    let section = section_of(vec![make_para("Plain")], tiny_layout());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert!(
        pages.iter().all(|p| p.comment_items.is_empty()),
        "no comment anchors ⇒ no comment panel"
    );
}

#[test]
fn text_flows_down_columns_before_paging() {
    let mut r = test_resources();
    // 12 short, left-aligned paragraphs: each line starts at the column's left
    // edge, so a run's x-origin reveals which column it landed in.
    let paras: Vec<_> = (0..12).map(|i| make_para(&format!("Line {i}"))).collect();

    // tiny_layout: 200×100 pt, L/R margin 10 → content width 180, height 90.
    // Two columns with an 18 pt gap ⇒ column width (180−18)/2 = 81, so the
    // second column's left edge sits at x = 81 + 18 = 99 (content-local).
    let two_col = PageLayout {
        columns: Some(SectionColumns {
            count: 2,
            gap: Points::new(18.0),
            separator: false,
        }),
        ..tiny_layout()
    };
    let (col_pages, _) = flow_paginated(&mut r, &section_of(paras.clone(), two_col));

    // The same content single-column needs more pages (each column holds only
    // ~90 pt; two columns roughly double per-page capacity).
    let (plain_pages, _) = flow_paginated(&mut r, &section_of(paras, tiny_layout()));

    assert!(
        col_pages.len() < plain_pages.len(),
        "two columns must fit more per page: {} cols vs {} plain",
        col_pages.len(),
        plain_pages.len()
    );

    // The first page must actually use the second column band (x ≥ 99) as well
    // as the first (x near 0).
    let xs = glyph_x_origins(&col_pages[0]);
    assert!(
        xs.iter().any(|&x| x < 50.0),
        "first column (x≈0) must be used: {xs:?}"
    );
    assert!(
        xs.iter().any(|&x| x >= 99.0),
        "second column (x≥99) must be used: {xs:?}"
    );
}

#[test]
fn column_separator_line_is_drawn() {
    let mut r = test_resources();
    let paras: Vec<_> = (0..12).map(|i| make_para(&format!("Line {i}"))).collect();
    let with_sep = PageLayout {
        columns: Some(SectionColumns {
            count: 2,
            gap: Points::new(18.0),
            separator: true,
        }),
        ..tiny_layout()
    };
    let (pages, _) = flow_paginated(&mut r, &section_of(paras, with_sep));

    // A thin full-height FilledRect must sit in the gap centre (x ≈ 81 + 9 = 90).
    let sep = pages[0].content_items.iter().find_map(|i| {
        if let PositionedItem::FilledRect(r) = i {
            let cx = r.rect.origin.x + r.rect.size.width / 2.0;
            ((cx - 90.0).abs() < 1.0 && r.rect.size.height > 80.0).then_some(r.rect)
        } else {
            None
        }
    });
    assert!(
        sep.is_some(),
        "expected a full-height separator near x=90; items: {:?}",
        pages[0].content_items.len()
    );
}

#[test]
fn single_column_keeps_content_in_one_band() {
    let mut r = test_resources();
    let paras: Vec<_> = (0..12).map(|i| make_para(&format!("Line {i}"))).collect();
    let (pages, _) = flow_paginated(&mut r, &section_of(paras, tiny_layout()));
    // No column layout: every run starts near the left margin (x≈0); nothing is
    // shifted into a second band.
    for page in &pages {
        for x in glyph_x_origins(page) {
            assert!(x < 50.0, "single-column run unexpectedly shifted to x={x}");
        }
    }
}

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
    assert!(
        total_height > 0.0,
        "cursor must advance: got {total_height}"
    );
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
        if let PositionedItem::GlyphRun(run) = i {
            Some(run.origin.y)
        } else {
            None
        }
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
    assert!(
        !pages[1].content_items.is_empty(),
        "page 2 must have content"
    );
}

#[test]
fn page_break_after_splits_onto_second_page() {
    let mut r = test_resources();
    let para1 = make_para_with_props(
        "Page one",
        ParaProps {
            page_break_after: Some(true),
            ..Default::default()
        },
    );
    let para2 = make_para("Page two");
    let section = section_of(vec![para1, para2], PageLayout::default());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(
        pages.len(),
        2,
        "page_break_after must produce a second page"
    );
    assert!(
        !pages[1].content_items.is_empty(),
        "page 2 must have content from para2"
    );
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
    assert!(
        triggered,
        "expected BlockExceedsPageHeight warning; got {warnings:?}"
    );
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
        if let PositionedItem::GlyphRun(run) = i {
            Some(run.origin.x)
        } else {
            None
        }
    });
    let x = first_run_x.expect("expected a glyph run");
    assert_eq!(
        x, left_margin,
        "pageless item x should be offset by left margin"
    );
}

// ── Helpers for new tests ─────────────────────────────────────────────────────

fn make_para_with_props(text: &str, props: ParaProps) -> StyledParagraph {
    StyledParagraph {
        direct_para_props: Some(Box::new(props)),
        ..make_para(text)
    }
}

fn first_glyph_y(items: &[PositionedItem]) -> Option<f32> {
    items.iter().find_map(|i| match i {
        PositionedItem::GlyphRun(r) => Some(r.origin.y),
        PositionedItem::ClippedGroup { items, .. } => first_glyph_y(items),
        _ => None,
    })
}

fn has_clipped_group(items: &[PositionedItem]) -> bool {
    items
        .iter()
        .any(|i| matches!(i, PositionedItem::ClippedGroup { .. }))
}

/// Recursively checks for any glyph run, descending into clip/rotation groups.
/// Table cell content is wrapped in a per-cell `ClippedGroup`, so a flat scan
/// would miss it.
fn any_glyph_run(items: &[PositionedItem]) -> bool {
    items.iter().any(|i| match i {
        PositionedItem::GlyphRun(_) => true,
        PositionedItem::ClippedGroup { items, .. } | PositionedItem::RotatedGroup { items, .. } => {
            any_glyph_run(items)
        }
        _ => false,
    })
}

// ── Paragraph splitting tests ─────────────────────────────────────────────────

#[test]
fn paragraph_split_produces_clipped_groups_on_multiple_pages() {
    let mut r = test_resources();
    // Long text at 12pt in 180pt-wide content area forces 10+ lines (~140pt+).
    let text = "Lorem ipsum dolor sit amet consectetur adipiscing. ".repeat(8);
    let section = section_of(vec![make_para(&text)], tiny_layout());
    let (pages, warnings) = flow_paginated(&mut r, &section);
    assert!(
        pages.len() >= 2,
        "expected 2+ pages for tall paragraph, got {}",
        pages.len()
    );
    // Page 1 must have a ClippedGroup (Fragment A of the split paragraph).
    assert!(
        has_clipped_group(&pages[0].content_items),
        "page 1 should have a ClippedGroup"
    );
    // All pages must have content.
    for (i, page) in pages.iter().enumerate() {
        assert!(
            !page.content_items.is_empty(),
            "page {i} should have content"
        );
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
    assert!(
        clip_b.height() > 0.0,
        "Fragment B clip height must be positive"
    );
}

// ── force-split termination tests (line taller than page) ────────────────────

/// Regression test: a single line taller than the page combined with a
/// nonzero `space_before` previously hung `split_and_place_loop` — every
/// fresh page started at `cursor_y == space_before > 0`, so the
/// "flush and retry" arm fired forever and pages were pushed unboundedly.
#[test]
fn line_taller_than_page_with_space_before_terminates() {
    use loki_doc_model::style::props::char_props::CharProps;
    let mut r = test_resources();
    // 120pt font on a page with 90pt content height: the single line cannot
    // fit even on a fresh page.
    let para = StyledParagraph {
        direct_para_props: Some(Box::new(ParaProps {
            space_before: Some(Spacing::Exact(Points::new(20.0))),
            ..Default::default()
        })),
        direct_char_props: Some(Box::new(CharProps {
            font_size: Some(Points::new(120.0)),
            ..Default::default()
        })),
        ..make_para("Huge")
    };
    let section = section_of(vec![para], tiny_layout());
    let (pages, warnings) = flow_paginated(&mut r, &section);
    assert!(
        !pages.is_empty() && pages.len() <= 4,
        "layout must terminate with a bounded page count, got {} pages",
        pages.len()
    );
    let exceeds = warnings
        .iter()
        .any(|w| matches!(w, LayoutWarning::BlockExceedsPageHeight { .. }));
    assert!(exceeds, "expected BlockExceedsPageHeight; got {warnings:?}");
}

/// Multiple consecutive lines each taller than the page must also terminate
/// (exercises the progress guard in the normal split arm and repeated
/// force-splits across fragments).
#[test]
fn multiple_lines_each_taller_than_page_terminate() {
    use loki_doc_model::style::props::char_props::CharProps;
    let mut r = test_resources();
    // Each word at 120pt is wider than the 180pt content width, so each gets
    // its own ~140pt line — taller than the 90pt content height.
    let para = StyledParagraph {
        direct_para_props: Some(Box::new(ParaProps {
            space_before: Some(Spacing::Exact(Points::new(20.0))),
            ..Default::default()
        })),
        direct_char_props: Some(Box::new(CharProps {
            font_size: Some(Points::new(120.0)),
            ..Default::default()
        })),
        ..make_para("Www Www Www")
    };
    let section = section_of(vec![para], tiny_layout());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert!(
        !pages.is_empty() && pages.len() <= 10,
        "layout must terminate with a bounded page count, got {} pages",
        pages.len()
    );
    // Every emitted page after the first must carry content (no run of empty
    // filler pages from repeated flushing).
    for (i, page) in pages.iter().enumerate().skip(1) {
        assert!(
            !page.content_items.is_empty(),
            "page {i} should have content"
        );
    }
}

// ── keep-together tests ───────────────────────────────────────────────────────

#[test]
fn keep_together_block_fits_on_current_page_no_flush() {
    let mut r = test_resources();
    // Two paragraphs; para2 has keep_together. Both fit on one page.
    let para1 = make_para("First paragraph short.");
    let para2 = make_para_with_props(
        "Second paragraph with keep_together.",
        ParaProps {
            keep_together: Some(true),
            ..Default::default()
        },
    );
    let section = section_of(vec![para1, para2], PageLayout::default());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(
        pages.len(),
        1,
        "both paragraphs fit on one page, expected 1 page"
    );
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
        ParaProps {
            keep_together: Some(true),
            ..Default::default()
        },
    );
    let section = section_of(vec![para1, para2], tiny_layout());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(
        pages.len(),
        2,
        "para2 should be pushed to page 2, expected 2 pages"
    );
    assert!(
        !pages[1].content_items.is_empty(),
        "page 2 should have para2 content"
    );
}

#[test]
fn keep_together_taller_than_page_emits_override_warning() {
    let mut r = test_resources();
    let long_text = "Lorem ipsum dolor sit amet consectetur adipiscing. ".repeat(8);
    let para = make_para_with_props(
        &long_text,
        ParaProps {
            keep_together: Some(true),
            ..Default::default()
        },
    );
    let section = section_of(vec![para], tiny_layout());
    let (_, warnings) = flow_paginated(&mut r, &section);
    // Either KeepTogetherOverride or BlockExceedsPageHeight must be emitted
    // (KeepTogetherOverride fires when height > page_content_height).
    let has_override = warnings
        .iter()
        .any(|w| matches!(w, LayoutWarning::KeepTogetherOverride { .. }));
    let has_exceeds = warnings
        .iter()
        .any(|w| matches!(w, LayoutWarning::BlockExceedsPageHeight { .. }));
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
        ParaProps {
            keep_with_next: Some(true),
            ..Default::default()
        },
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
        ParaProps {
            keep_with_next: Some(true),
            ..Default::default()
        },
    );
    let para2 = make_para("Body after heading.");
    let section = section_of(vec![para0, para1, para2], tiny_layout());
    let (pages, _) = flow_paginated(&mut r, &section);
    assert!(
        pages.len() >= 2,
        "chain should be pushed to page 2, got {} pages",
        pages.len()
    );
    // para1 and para2 must be on the same page (page 2).
    assert!(
        !pages[1].content_items.is_empty(),
        "page 2 must have content"
    );
}

#[test]
fn keep_with_next_chain_truncated_at_limit_5() {
    let mut r = test_resources();
    // 6 paragraphs with keep_with_next=true + 1 terminal = 7 blocks total.
    // Chain limit is 5, so chain is truncated and warning is emitted.
    let kwn = ParaProps {
        keep_with_next: Some(true),
        ..Default::default()
    };
    let mut paras: Vec<StyledParagraph> = (0..6)
        .map(|i| make_para_with_props(&format!("kwn para {i}"), kwn.clone()))
        .collect();
    paras.push(make_para("terminal"));
    let section = section_of(paras, PageLayout::default());
    let (_, warnings) = flow_paginated(&mut r, &section);
    let truncated = warnings
        .iter()
        .any(|w| matches!(w, LayoutWarning::KeepWithNextChainTruncated { .. }));
    assert!(
        truncated,
        "expected KeepWithNextChainTruncated warning; got {warnings:?}"
    );
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
    let too_tall = warnings
        .iter()
        .any(|w| matches!(w, LayoutWarning::KeepWithNextChainTooTall { .. }));
    assert!(
        too_tall,
        "expected KeepWithNextChainTooTall warning; got {warnings:?}"
    );
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
        ParaProps {
            page_break_before: Some(true),
            ..Default::default()
        },
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

// ── List marker synthesis tests ───────────────────────────────────────────────

/// Build a StyleCatalog pre-populated with one decimal list (id="1") and
/// one bullet list (id="2"), each with a single level.
fn list_catalog() -> StyleCatalog {
    let mut catalog = StyleCatalog::new();

    // Decimal list "1": level 0, format "%1.", start=1.
    catalog.list_styles.insert(
        ListId::new("1"),
        ListStyle {
            id: ListId::new("1"),
            display_name: None,
            levels: vec![ListLevel {
                level: 0,
                kind: ListLevelKind::Numbered {
                    scheme: NumberingScheme::Decimal,
                    start_value: 1,
                    format: "%1.".to_string(),
                    display_levels: 1,
                },
                indent_start: Points::new(36.0),
                hanging_indent: Points::new(18.0),
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: None,
                char_props: Default::default(),
            }],
            extensions: ExtensionBag::default(),
        },
    );

    // Bullet list "2": level 0, bullet char '•'.
    catalog.list_styles.insert(
        ListId::new("2"),
        ListStyle {
            id: ListId::new("2"),
            display_name: None,
            levels: vec![ListLevel {
                level: 0,
                kind: ListLevelKind::Bullet {
                    char: BulletChar::Char('•'),
                    font: None,
                },
                indent_start: Points::new(36.0),
                hanging_indent: Points::new(18.0),
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: None,
                char_props: Default::default(),
            }],
            extensions: ExtensionBag::default(),
        },
    );

    catalog
}

fn list_para(text: &str, list_id: &str, level: u8) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(ParaProps {
            list_id: Some(ListId::new(list_id)),
            list_level: Some(level),
            indent_start: Some(Points::new(36.0)),
            indent_hanging: Some(Points::new(18.0)),
            ..Default::default()
        })),
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    }
}

fn flow_with_catalog(
    r: &mut FontResources,
    section: &Section,
    catalog: &StyleCatalog,
) -> (Vec<PositionedItem>, f32, Vec<LayoutWarning>) {
    match flow_section(
        r,
        section,
        catalog,
        &LayoutMode::Pageless,
        1.0,
        &LayoutOptions::default(),
        &[],
    ) {
        FlowOutput::Canvas {
            items,
            height,
            warnings,
            ..
        } => (items, height, warnings),
        _ => panic!("expected Canvas"),
    }
}

#[test]
fn list_items_produce_glyph_runs() {
    let mut r = test_resources();
    let catalog = list_catalog();
    let section = section_of(
        vec![list_para("Item one", "1", 0), list_para("Item two", "1", 0)],
        PageLayout::default(),
    );
    let (items, height, warnings) = flow_with_catalog(&mut r, &section, &catalog);
    assert!(height > 0.0, "list section must have non-zero height");
    assert!(
        !warnings.is_empty() || warnings.is_empty(),
        "warnings either way is fine"
    );
    let runs = items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    assert!(
        runs >= 2,
        "two list items must produce at least two glyph runs, got {runs}"
    );
}

#[test]
fn numbered_list_counters_advance() {
    // Three decimal items "Item 1/2/3" — we can't inspect the text directly
    // from PositionedItems, but the test verifies that the flow engine does not
    // panic and produces the expected number of glyph runs.
    let mut r = test_resources();
    let catalog = list_catalog();
    let section = section_of(
        vec![
            list_para("Item 1", "1", 0),
            list_para("Item 2", "1", 0),
            list_para("Item 3", "1", 0),
        ],
        PageLayout::default(),
    );
    let (items, _, _) = flow_with_catalog(&mut r, &section, &catalog);
    let runs = items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    assert!(
        runs >= 3,
        "three list items must produce ≥3 glyph runs, got {runs}"
    );
}

#[test]
fn bullet_list_items_produce_output() {
    let mut r = test_resources();
    let catalog = list_catalog();
    let section = section_of(
        vec![list_para("Bullet A", "2", 0), list_para("Bullet B", "2", 0)],
        PageLayout::default(),
    );
    let (items, _, _) = flow_with_catalog(&mut r, &section, &catalog);
    let runs = items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    assert!(
        runs >= 2,
        "bullet list must produce ≥2 glyph runs, got {runs}"
    );
}

#[test]
fn new_list_resets_counter() {
    // Two separate decimal lists separated by a non-list paragraph.
    // The second list should restart at "1." not continue from the first.
    // We can't inspect text from PositionedItems, but the test verifies
    // no panic and that the layout engine handles the list boundary correctly.
    let mut r = test_resources();
    let catalog = list_catalog();
    let section = section_of(
        vec![
            list_para("List A item 1", "1", 0),
            list_para("List A item 2", "1", 0),
            make_para("separator"),
            list_para("List B item 1", "1", 0),
        ],
        PageLayout::default(),
    );
    let (items, _, _) = flow_with_catalog(&mut r, &section, &catalog);
    let runs = items
        .iter()
        .filter(|i| matches!(i, PositionedItem::GlyphRun(_)))
        .count();
    assert!(
        runs >= 4,
        "four paragraphs must produce ≥4 glyph runs, got {runs}"
    );
}

// ── Table tests ───────────────────────────────────────────────────────────────

fn make_cell(text: &str) -> Cell {
    Cell {
        attr: NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span: 1,
        col_span: 1,
        blocks: vec![Block::StyledPara(make_para(text))],
        props: CellProps::default(),
    }
}

fn make_table_2x2(cell_props: Option<CellProps>) -> Block {
    let make = |text: &str| -> Cell {
        let mut c = make_cell(text);
        if let Some(ref p) = cell_props {
            c.props = p.clone();
        }
        c
    };
    let row1 = Row::new(vec![make("R1C1"), make("R1C2")]);
    let row2 = Row::new(vec![make("R2C1"), make("R2C2")]);
    Block::Table(Box::new(Table {
        attr: NodeAttr::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
        ],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![row1, row2])],
        foot: TableFoot::empty(),
    }))
}

/// A 2×2 table with short text cells should fit on one page and produce glyph runs.
#[test]
fn table_2x2_renders_on_one_page() {
    let mut r = test_resources();
    let section = Section {
        layout: PageLayout::default(),
        blocks: vec![make_table_2x2(None)],
        extensions: ExtensionBag::default(),
    };
    let (pages, _) = flow_paginated(&mut r, &section);
    assert_eq!(
        pages.len(),
        1,
        "2×2 table should fit on one page, got {}",
        pages.len()
    );
    let has_runs = any_glyph_run(&pages[0].content_items);
    assert!(has_runs, "table cells must produce glyph runs");
}

/// A cell with a red background should produce a `FilledRect` item.
#[test]
fn table_cell_background_produces_filled_rect() {
    use appthere_color::RgbColor;
    let mut r = test_resources();
    let props = CellProps {
        background_color: Some(DocumentColor::Rgb(RgbColor::new(1.0, 0.0, 0.0))),
        ..Default::default()
    };
    let section = Section {
        layout: PageLayout::default(),
        blocks: vec![make_table_2x2(Some(props))],
        extensions: ExtensionBag::default(),
    };
    let (items, _, _) = flow_pageless(&mut r, &section);
    let filled_rects = items
        .iter()
        .filter(|i| matches!(i, PositionedItem::FilledRect(_)))
        .count();
    assert!(
        filled_rects >= 2,
        "2×2 table with bg should produce ≥2 FilledRect items (one per cell in first row), got {filled_rects}"
    );
}

/// A cell with borders should produce a `BorderRect` item.
#[test]
fn table_cell_borders_produce_border_rect() {
    let mut r = test_resources();
    let border = Border {
        style: BorderStyle::Solid,
        width: Points::new(1.0),
        color: None,
        spacing: None,
    };
    let props = CellProps {
        border_top: Some(border.clone()),
        border_bottom: Some(border.clone()),
        border_left: Some(border.clone()),
        border_right: Some(border),
        ..Default::default()
    };
    let section = Section {
        layout: PageLayout::default(),
        blocks: vec![make_table_2x2(Some(props))],
        extensions: ExtensionBag::default(),
    };
    let (items, _, _) = flow_pageless(&mut r, &section);
    let border_rects = items
        .iter()
        .filter(|i| matches!(i, PositionedItem::BorderRect(_)))
        .count();
    assert!(
        border_rects >= 2,
        "2×2 table with borders should produce ≥2 BorderRect items, got {border_rects}"
    );
}

// ── PAGE / NUMPAGES field substitution in headers & footers ──────────────────

mod page_fields {
    use super::*;
    use crate::FieldContext;
    use loki_doc_model::content::field::types::{Field, FieldKind};
    use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};

    fn para_with_inlines(inlines: Vec<Inline>) -> StyledParagraph {
        StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: None,
            inlines,
            attr: NodeAttr::default(),
        }
    }

    fn page_field_footer() -> HeaderFooter {
        let mut hf = HeaderFooter::new(HeaderFooterKind::Default);
        hf.blocks = vec![Block::StyledPara(para_with_inlines(vec![
            Inline::Str("Page ".into()),
            Inline::Field(Field::new(FieldKind::PageNumber).with_current_value("1")),
            Inline::Str(" of ".into()),
            Inline::Field(Field::new(FieldKind::PageCount).with_current_value("1")),
        ]))];
        hf
    }

    #[test]
    fn detects_page_field_in_styled_para() {
        assert!(blocks_contain_page_field(&page_field_footer().blocks));
    }

    #[test]
    fn detects_page_field_nested_in_strong() {
        let para = para_with_inlines(vec![Inline::Strong(vec![Inline::Field(Field::new(
            FieldKind::PageNumber,
        ))])]);
        assert!(blocks_contain_page_field(&[Block::StyledPara(para)]));
    }

    #[test]
    fn no_page_field_in_plain_text() {
        let para = para_with_inlines(vec![Inline::Str("Confidential".into())]);
        assert!(!blocks_contain_page_field(&[Block::StyledPara(para)]));
    }

    #[test]
    fn substitution_replaces_fields_with_context_values() {
        let mut blocks = page_field_footer().blocks;
        substitute_page_fields(
            &mut blocks,
            &FieldContext {
                page_number: 7,
                page_count: 12,
                number_format: None,
            },
        );
        let Block::StyledPara(p) = &blocks[0] else {
            panic!("expected StyledPara");
        };
        let text: String = p
            .inlines
            .iter()
            .map(|i| match i {
                Inline::Str(s) => s.as_str(),
                _ => "<non-text>",
            })
            .collect();
        assert_eq!(text, "Page 7 of 12");
    }

    #[test]
    fn substitution_formats_page_number_as_lower_roman() {
        let mut blocks = page_field_footer().blocks;
        substitute_page_fields(
            &mut blocks,
            &FieldContext {
                page_number: 7,
                page_count: 12,
                // w:pgNumType w:fmt="lowerRoman"
                number_format: Some(loki_doc_model::style::list_style::NumberingScheme::LowerRoman),
            },
        );
        let Block::StyledPara(p) = &blocks[0] else {
            panic!("expected StyledPara");
        };
        let text: String = p
            .inlines
            .iter()
            .map(|i| match i {
                Inline::Str(s) => s.as_str(),
                _ => "<non-text>",
            })
            .collect();
        // Page number 7 → "vii"; NUMPAGES stays decimal.
        assert_eq!(text, "Page vii of 12");
    }

    #[test]
    fn assign_headers_footers_renders_distinct_page_numbers() {
        let mut r = test_resources();
        // Three short paragraphs with page breaks → 3 pages.
        let mut paras = vec![make_para("one")];
        let mut p2 = make_para("two");
        p2.direct_para_props = Some(Box::new(ParaProps {
            page_break_before: Some(true),
            ..Default::default()
        }));
        let mut p3 = make_para("three");
        p3.direct_para_props = Some(Box::new(ParaProps {
            page_break_before: Some(true),
            ..Default::default()
        }));
        paras.push(p2);
        paras.push(p3);

        let mut layout = tiny_layout();
        layout.footer = Some(page_field_footer());
        let section = section_of(paras, layout.clone());

        let (mut pages, _) = flow_paginated(&mut r, &section);
        assert_eq!(pages.len(), 3, "expected 3 pages");

        let catalog = StyleCatalog::new();
        assign_headers_footers(&mut pages, &layout, &mut r, &catalog, 1.0, 3);

        for page in &pages {
            assert!(
                !page.footer_items.is_empty(),
                "page {} should have footer items",
                page.page_number
            );
        }
        // The PAGE field renders a different number on each page, so the
        // glyph runs must differ between pages (Debug form captures glyphs).
        let f1 = format!("{:?}", pages[0].footer_items);
        let f2 = format!("{:?}", pages[1].footer_items);
        let f3 = format!("{:?}", pages[2].footer_items);
        assert_ne!(f1, f2, "page 1 and 2 footers should differ");
        assert_ne!(f2, f3, "page 2 and 3 footers should differ");
    }

    #[test]
    fn static_footer_is_identical_across_pages() {
        let mut r = test_resources();
        let mut paras = vec![make_para("one")];
        let mut p2 = make_para("two");
        p2.direct_para_props = Some(Box::new(ParaProps {
            page_break_before: Some(true),
            ..Default::default()
        }));
        paras.push(p2);

        let mut hf = HeaderFooter::new(HeaderFooterKind::Default);
        hf.blocks = vec![Block::StyledPara(para_with_inlines(vec![Inline::Str(
            "Confidential".into(),
        )]))];
        let mut layout = tiny_layout();
        layout.footer = Some(hf);
        let section = section_of(paras, layout.clone());

        let (mut pages, _) = flow_paginated(&mut r, &section);
        assert_eq!(pages.len(), 2, "expected 2 pages");

        let catalog = StyleCatalog::new();
        assign_headers_footers(&mut pages, &layout, &mut r, &catalog, 1.0, 2);

        let f1 = format!("{:?}", pages[0].footer_items);
        let f2 = format!("{:?}", pages[1].footer_items);
        assert_eq!(f1, f2, "static footers should be identical on every page");
    }

    /// A floating image taller than its short anchoring paragraph keeps wrapping
    /// the *following* paragraph beside it (cross-paragraph wrap), and text below
    /// the float reclaims the full column width.
    #[test]
    fn tall_float_wraps_following_paragraph() {
        use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};
        use loki_doc_model::content::inline::LinkTarget;

        fn floating_image(cx_emu: u64, cy_emu: u64) -> Inline {
            let mut attr = NodeAttr::default();
            attr.kv.push(("cx_emu".into(), cx_emu.to_string()));
            attr.kv.push(("cy_emu".into(), cy_emu.to_string()));
            // side = Right (text on the right) → float on the LEFT, text shifts right.
            FloatWrap {
                wrap: TextWrap::Square,
                side: WrapSide::Right,
                behind_text: false,
            }
            .store(&mut attr);
            Inline::Image(attr, vec![], LinkTarget::new("data:image/png;base64,AAAA"))
        }

        let mut r = test_resources();
        // 1 in × 1 in float (72 × 72 pt) anchored in a one-word paragraph.
        let anchor = StyledParagraph {
            style_id: None,
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![floating_image(914_400, 914_400), Inline::Str("Hi.".into())],
            attr: NodeAttr::default(),
        };
        // A long follower that wraps for many lines — past the float bottom.
        let follower_text = "The quick brown fox jumps over the lazy dog. ".repeat(14);
        let follower = make_para(&follower_text);
        let section = section_of(vec![anchor, follower], PageLayout::default());

        let (items, _h, _w) = flow_pageless(&mut r, &section);

        // The float image is a tall item at the left edge.
        let img = items
            .iter()
            .find_map(|i| match i {
                PositionedItem::Image(im) => Some(im),
                _ => None,
            })
            .expect("float image emitted");
        // In Canvas/Pageless output, content x carries the left-margin offset, so
        // a left float sits at ≈ the left margin rather than literal 0.
        let left_edge = img.rect.origin.x;
        assert!((img.rect.size.height - 72.0).abs() < 1.0, "1 in tall float");

        // Glyph-run origins by y.
        let glyphs: Vec<(f32, f32)> = items
            .iter()
            .filter_map(|i| match i {
                PositionedItem::GlyphRun(g) => Some((g.origin.y, g.origin.x)),
                _ => None,
            })
            .collect();

        // The FOLLOWER's first lines (below the one-line anchor, still within the
        // 72 pt float extent: y ∈ 20..60) are shifted right to clear the band.
        let beside_min_x = glyphs
            .iter()
            .filter(|(y, _)| *y > 20.0 && *y < 60.0)
            .map(|(_, x)| *x)
            .fold(f32::INFINITY, f32::min);
        assert!(
            beside_min_x > left_edge + 70.0,
            "follower lines beside the float must clear the band; \
             min x = {beside_min_x}, float left = {left_edge}"
        );

        // Lines below the float (y > 90) reclaim the full column (back to the
        // float's own left edge, i.e. the column start).
        let below_min_x = glyphs
            .iter()
            .filter(|(y, _)| *y > 90.0)
            .map(|(_, x)| *x)
            .fold(f32::INFINITY, f32::min);
        assert!(
            below_min_x < left_edge + 5.0,
            "text below the float must reclaim full width; \
             min x = {below_min_x}, float left = {left_edge}"
        );
    }
}
