// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::sync::Arc;

use loki_layout::{
    layout_paragraph, FontResources, LayoutColor, LayoutInsets, LayoutPage, LayoutSize,
    PageEditingData, PageParagraphData, PaginatedLayout, ResolvedParaProps, StyleSpan,
};

use super::{hit_test_document, hit_test_page};

/// Build a minimal `PaginatedLayout` with a single page containing one
/// paragraph placed at the content-area origin.
fn make_test_layout() -> PaginatedLayout {
    let mut resources = FontResources::new();
    let para = layout_paragraph(
        &mut resources,
        "Hello world",
        &[StyleSpan {
            range: 0..11,
            font_name: None,
            font_size: 12.0,
            bold: false,
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
        true, // preserve_for_editing
    );
    let editing_data = PageEditingData {
        paragraphs: vec![PageParagraphData {
            block_index: 0,
            layout: Arc::new(para),
            origin: (0.0, 0.0),
        }],
    };
    let page_size = LayoutSize::new(595.0, 842.0);
    let margins = LayoutInsets {
        top: 72.0,
        right: 72.0,
        bottom: 72.0,
        left: 72.0,
    };
    let page = LayoutPage {
        page_number: 1,
        page_size,
        margins,
        content_items: vec![],
        header_items: vec![],
        footer_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: Some(editing_data),
    };
    PaginatedLayout {
        page_size,
        pages: vec![page],
    }
}

/// Convert layout points to CSS pixels (inverse of PX_TO_PT).
fn pt_to_px(pt: f32) -> f32 {
    pt * (96.0 / 72.0)
}

/// canvas_origin + margin offset in CSS pixels.
fn canvas_origin_for_test() -> (f32, f32) {
    (0.0, 0.0)
}

#[test]
fn click_at_content_origin_returns_page0_para0_offset0() {
    let layout = make_test_layout();
    let page = &layout.pages[0];

    // Click at the content area's (0, 0): canvas_x = margin_left, canvas_y = margin_top.
    let page_w_px = pt_to_px(page.page_size.width);
    let page_h_px = pt_to_px(page.page_size.height);
    let margin_left_px = pt_to_px(page.margins.left);
    let margin_top_px = pt_to_px(page.margins.top);

    let result = hit_test_document(
        margin_left_px, // client_x = canvas_x = margin_left in px
        margin_top_px,  // client_y = canvas_y = margin_top in px
        canvas_origin_for_test(),
        0.0, // scroll_offset
        &layout,
        page_w_px,
        page_h_px,
        pt_to_px(24.0), // page_gap_px
    );
    let pos = result.expect("click at content origin should hit para 0");
    assert_eq!(pos.page_index, 0);
    assert_eq!(pos.paragraph_index, 0);
    assert_eq!(pos.byte_offset, 0, "top-left click should land at byte 0");
}

#[test]
fn click_below_all_content_returns_none() {
    let layout = make_test_layout();
    let page = &layout.pages[0];
    let page_h_px = pt_to_px(page.page_size.height);
    let page_w_px = pt_to_px(page.page_size.width);

    // Click far below the page canvas.
    let result = hit_test_document(
        page_w_px / 2.0,
        page_h_px + 100.0, // in the inter-page gap
        canvas_origin_for_test(),
        0.0,
        &layout,
        page_w_px,
        page_h_px,
        pt_to_px(24.0),
    );
    assert!(
        result.is_none(),
        "click below page content area must return None"
    );
}

#[test]
fn click_on_page2_returns_page_index_1() {
    let layout = {
        // Build a two-page layout by duplicating the single-page layout.
        let single = make_test_layout();
        let page0 = single.pages[0].clone();
        let mut page1 = page0.clone();
        page1.page_number = 2;
        PaginatedLayout {
            page_size: single.page_size,
            pages: vec![page0, page1],
        }
    };
    let page_h_px = pt_to_px(layout.page_size.height);
    let page_w_px = pt_to_px(layout.page_size.width);
    let page_gap_px = pt_to_px(24.0);
    let page = &layout.pages[1];
    let margin_left_px = pt_to_px(page.margins.left);
    // y at the content area of page 1 = page_height + gap + margin_top.
    let page2_margin_top_px = pt_to_px(page.margins.top);
    let click_y = page_h_px + page_gap_px + page2_margin_top_px;

    let result = hit_test_document(
        margin_left_px,
        click_y,
        canvas_origin_for_test(),
        0.0,
        &layout,
        page_w_px,
        page_h_px,
        page_gap_px,
    );
    let pos = result.expect("click on page 2 should succeed");
    assert_eq!(pos.page_index, 1, "should land on page 1 (0-based)");
}

/// Verifies that a negative canvas_y (which occurs when scroll_offset is not
/// subtracted from page_top_y in the click handler) causes hit_test_page to
/// return None.  This documents the root cause of the multi-page cursor bug
/// when scroll_offset is zero but the user has scrolled.
#[test]
fn hit_test_page_negative_y_returns_none() {
    let layout = make_test_layout();
    // y < 0 means the click is above the page canvas — should return None.
    let result = hit_test_page(0, 100.0, -10.0, &layout);
    assert!(result.is_none(), "negative y_in_page must return None");
}

/// Verifies that passing the correct scroll_offset to hit_test_document
/// allows a click on page 2 to be resolved when the user has scrolled.
///
/// This tests the mathematical contract of the coordinate transform, not
/// Blitz scroll tracking (which is currently unimplemented — see
/// TODO(partial-render) in editor.rs).
#[test]
fn scroll_offset_corrects_page2_click() {
    let layout = {
        let single = make_test_layout();
        let page0 = single.pages[0].clone();
        let mut page1 = page0.clone();
        page1.page_number = 2;
        PaginatedLayout {
            page_size: single.page_size,
            pages: vec![page0, page1],
        }
    };
    let page = &layout.pages[0];
    let page_h_px = pt_to_px(page.page_size.height);
    let page_w_px = pt_to_px(page.page_size.width);
    let page_gap_px = pt_to_px(24.0);
    let margin_left_px = pt_to_px(page.margins.left);
    let margin_top_px = pt_to_px(page.margins.top);

    // User has scrolled so that page 2 is at the top of the viewport.
    let scroll_offset = page_h_px + page_gap_px;

    // With this scroll, a click at client_y = canvas_origin.y + margin_top
    // should resolve to the top-left content area of page 2.
    let canvas_origin = canvas_origin_for_test();
    let click_y = canvas_origin.1 + margin_top_px;

    let result = hit_test_document(
        margin_left_px,
        click_y,
        canvas_origin,
        scroll_offset,
        &layout,
        page_w_px,
        page_h_px,
        page_gap_px,
    );
    let pos = result.expect("correct scroll_offset must resolve page 2 click");
    assert_eq!(
        pos.page_index, 1,
        "scroll-adjusted click must land on page 1 (0-based)"
    );
}

/// Verifies that omitting scroll_offset (passing 0) for a click that should
/// land on page 2 returns None or lands on the wrong page — confirming that
/// scroll_offset is required for correct multi-page hit testing.
#[test]
fn missing_scroll_offset_misses_page2_click() {
    let layout = {
        let single = make_test_layout();
        let page0 = single.pages[0].clone();
        let mut page1 = page0.clone();
        page1.page_number = 2;
        PaginatedLayout {
            page_size: single.page_size,
            pages: vec![page0, page1],
        }
    };
    let page = &layout.pages[0];
    let page_h_px = pt_to_px(page.page_size.height);
    let page_w_px = pt_to_px(page.page_size.width);
    let page_gap_px = pt_to_px(24.0);
    let margin_left_px = pt_to_px(page.margins.left);
    let margin_top_px = pt_to_px(page.margins.top);

    // Same scenario as above but scroll_offset is incorrectly left as 0.
    let canvas_origin = canvas_origin_for_test();
    let click_y = canvas_origin.1 + margin_top_px; // top of viewport when scrolled to page 2

    let result = hit_test_document(
        margin_left_px,
        click_y,
        canvas_origin,
        0.0, // wrong: no scroll_offset applied
        &layout,
        page_w_px,
        page_h_px,
        page_gap_px,
    );
    // Without scroll_offset, click_y maps to page 0 content (near top),
    // so result is either page 0 or None — never page 1.
    if let Some(pos) = result {
        assert_ne!(
            pos.page_index, 1,
            "without scroll_offset, click must not reach page 1"
        );
    }
}
