// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `result`.

use super::*;
use crate::color::LayoutColor;
use crate::geometry::LayoutRect;
use crate::items::PositionedRect;

fn make_filled(x: f32) -> PositionedItem {
    PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(x, 0.0, 10.0, 10.0),
        color: LayoutColor::BLACK,
    })
}

#[test]
fn continuous_all_items_count() {
    let layout = DocumentLayout::Continuous(ContinuousLayout {
        content_width: 500.0,
        total_height: 200.0,
        items: vec![make_filled(0.0), make_filled(20.0), make_filled(40.0)],
        paragraphs: vec![],
    });
    assert_eq!(layout.all_items().count(), 3);
}

fn para(text: &str, block_index: usize, origin: (f32, f32)) -> PageParagraphData {
    use crate::font::FontResources;
    use crate::para::{ResolvedParaProps, StyleSpan, layout_paragraph};
    let mut resources = FontResources::new();
    let layout = layout_paragraph(
        &mut resources,
        text,
        &[StyleSpan {
            range: 0..text.len(),
            font_name: None,
            font_size: 12.0,
            bold: false,
            weight: 400,
            italic: false,
            color: LayoutColor::BLACK,
            underline: None,
            strikethrough: None,
            line_height: None,
            vertical_align: None,
            highlight_color: None,
            letter_spacing: None,
            font_variant: None,
            word_spacing: None,
            shadow: false,
            link_url: None,
        }],
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true,
    );
    PageParagraphData {
        block_index,
        layout: Arc::new(layout),
        origin,
    }
}

fn two_para_continuous() -> ContinuousLayout {
    let p0 = para("Hello world", 0, (0.0, 0.0));
    let h0 = p0.layout.height;
    let p1 = para("Second line here", 1, (0.0, h0));
    ContinuousLayout {
        content_width: 400.0,
        total_height: h0 + p1.layout.height,
        items: vec![],
        paragraphs: vec![p0, p1],
    }
}

#[test]
fn selection_rects_collapsed_is_empty() {
    let cl = two_para_continuous();
    assert!(cl.selection_rects((0, 3), (0, 3)).is_empty());
}

#[test]
fn selection_rects_within_paragraph() {
    let cl = two_para_continuous();
    let rects = cl.selection_rects((0, 0), (0, 5));
    assert!(!rects.is_empty(), "expected highlight rects");
    // Confined to the first paragraph (origin y = 0, near the top).
    assert!(rects.iter().all(|r| r.origin.y < 30.0));
}

#[test]
fn selection_rects_span_two_paragraphs() {
    let cl = two_para_continuous();
    // Split at the boundary midpoint; line ascent puts a rect's top a point
    // or so above the nominal paragraph origin, so an exact `>= origin`
    // comparison is too strict.
    let mid = cl.paragraphs[1].origin.1 / 2.0;
    let rects = cl.selection_rects((0, 6), (1, 6));
    // Endpoint order is normalised, so reversing gives the same result.
    let rev = cl.selection_rects((1, 6), (0, 6));
    assert_eq!(rects.len(), rev.len());
    assert!(rects.iter().any(|r| r.origin.y < mid)); // first paragraph
    assert!(rects.iter().any(|r| r.origin.y > mid)); // second paragraph
}

#[test]
fn hit_test_and_cursor_round_trip() {
    let cl = two_para_continuous();
    // A click on the second paragraph resolves to block 1.
    let (block, _byte) = cl
        .hit_test(2.0, cl.paragraphs[1].origin.1 + 2.0)
        .expect("hit");
    assert_eq!(block, 1);
    // Caret for the second paragraph sits at/after its canvas origin.
    let cr = cl.cursor_rect_canvas(1, 0).expect("caret");
    assert!(cr.y >= cl.paragraphs[1].origin.1 - 1.0);
}

#[test]
fn paginated_all_items_across_pages() {
    let page1 = LayoutPage {
        page_number: 1,
        page_size: LayoutSize::new(595.0, 842.0),
        margins: LayoutInsets::uniform(72.0),
        content_items: vec![make_filled(0.0), make_filled(10.0)],
        header_items: vec![make_filled(20.0)],
        footer_items: vec![],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    };
    let page2 = LayoutPage {
        page_number: 2,
        page_size: LayoutSize::new(595.0, 842.0),
        margins: LayoutInsets::uniform(72.0),
        content_items: vec![make_filled(0.0)],
        header_items: vec![],
        footer_items: vec![make_filled(30.0)],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    };
    let layout = DocumentLayout::Paginated(PaginatedLayout {
        page_size: LayoutSize::new(595.0, 842.0),
        pages: vec![Arc::new(page1), Arc::new(page2)],
    });
    // page1: 2 content + 1 header = 3; page2: 1 content + 1 footer = 2 → total 5
    assert_eq!(layout.all_items().count(), 5);
}

#[test]
fn layout_page_content_rect() {
    let page = LayoutPage {
        page_number: 1,
        page_size: LayoutSize::new(595.0, 842.0),
        margins: LayoutInsets {
            top: 72.0,
            right: 72.0,
            bottom: 72.0,
            left: 72.0,
        },
        content_items: vec![],
        header_items: vec![],
        footer_items: vec![],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    };
    let cr = page.content_rect();
    assert_eq!(cr.x(), 72.0);
    assert_eq!(cr.y(), 72.0);
    assert_eq!(cr.width(), 595.0 - 144.0);
    assert_eq!(cr.height(), 842.0 - 144.0);
}

#[test]
fn document_layout_total_height_paginated() {
    let make_page = |n: usize| LayoutPage {
        page_number: n,
        page_size: LayoutSize::new(595.0, 842.0),
        margins: LayoutInsets::uniform(72.0),
        content_items: vec![],
        header_items: vec![],
        footer_items: vec![],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    };
    let layout = DocumentLayout::Paginated(PaginatedLayout {
        page_size: LayoutSize::new(595.0, 842.0),
        pages: vec![Arc::new(make_page(1)), Arc::new(make_page(2))],
    });
    assert_eq!(layout.total_height(), 842.0 * 2.0);
}

#[test]
fn document_layout_content_width_continuous() {
    let layout = DocumentLayout::Continuous(ContinuousLayout {
        content_width: 480.0,
        total_height: 100.0,
        items: vec![],
        paragraphs: vec![],
    });
    assert_eq!(layout.content_width(), 480.0);
}
