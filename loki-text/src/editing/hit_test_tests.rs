// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::sync::Arc;

use loki_layout::{
    FontResources, LayoutColor, LayoutInsets, LayoutPage, LayoutSize, PageEditingData,
    PageParagraphData, PaginatedLayout, ResolvedParaProps, StyleSpan, layout_paragraph,
};

use super::*;

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
            math: None,
            scale: None,
            kerning: None,
            baseline_shift: None,
        }],
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true, // preserve_for_editing
    );
    let editing_data = PageEditingData {
        paragraphs: vec![PageParagraphData {
            block_index: 0,
            path: Vec::new(),
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
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: Some(editing_data),
    };
    PaginatedLayout {
        page_size,
        pages: vec![Arc::new(page)],
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
        1.0,            // zoom
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
        1.0, // zoom
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
        let mut page1 = (*page0).clone();
        page1.page_number = 2;
        PaginatedLayout {
            page_size: single.page_size,
            pages: vec![page0, Arc::new(page1)],
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
        1.0, // zoom
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
        let mut page1 = (*page0).clone();
        page1.page_number = 2;
        PaginatedLayout {
            page_size: single.page_size,
            pages: vec![page0, Arc::new(page1)],
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
        1.0, // zoom
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
        let mut page1 = (*page0).clone();
        page1.page_number = 2;
        PaginatedLayout {
            page_size: single.page_size,
            pages: vec![page0, Arc::new(page1)],
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
        1.0, // zoom
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

/// At zoom ≠ 1.0 the painted tiles are scaled, so a window-relative click must
/// be de-scaled before mapping to layout points. A click at the content origin
/// scaled by the zoom must still resolve to para 0, byte 0.
#[test]
fn zoomed_click_at_content_origin_resolves_para0() {
    let layout = make_test_layout();
    let page = &layout.pages[0];
    let page_w_px = pt_to_px(page.page_size.width);
    let page_h_px = pt_to_px(page.page_size.height);
    let zoom = 2.0;
    // The painted content origin is the (unzoomed) margin offset scaled by zoom.
    let click_x = pt_to_px(page.margins.left) * zoom;
    let click_y = pt_to_px(page.margins.top) * zoom;

    let result = hit_test_document(
        click_x,
        click_y,
        canvas_origin_for_test(),
        0.0,
        &layout,
        page_w_px,
        page_h_px,
        pt_to_px(24.0),
        zoom,
    );
    let pos = result.expect("zoomed click at content origin should hit para 0");
    assert_eq!(pos.page_index, 0);
    assert_eq!(pos.paragraph_index, 0);
    assert_eq!(pos.byte_offset, 0, "top-left click should land at byte 0");
}

/// The page stride is `page_height × zoom + gap` (the gap is a fixed CSS margin,
/// unscaled). A zoomed click at page 2's content origin must resolve to page
/// index 1 — the pre-fix code divided by an unzoomed stride and over-counted.
#[test]
fn zoomed_click_on_page2_resolves_page_index_1() {
    let layout = {
        let single = make_test_layout();
        let page0 = single.pages[0].clone();
        let mut page1 = (*page0).clone();
        page1.page_number = 2;
        PaginatedLayout {
            page_size: single.page_size,
            pages: vec![page0, Arc::new(page1)],
        }
    };
    let page = &layout.pages[1];
    let page_h_px = pt_to_px(layout.page_size.height);
    let page_w_px = pt_to_px(layout.page_size.width);
    let page_gap_px = pt_to_px(24.0);
    let zoom = 2.0;
    // Page 1's top is one scaled page height + one (unscaled) gap down; its
    // content origin adds the scaled top margin.
    let click_x = pt_to_px(page.margins.left) * zoom;
    let click_y = page_h_px * zoom + page_gap_px + pt_to_px(page.margins.top) * zoom;

    let result = hit_test_document(
        click_x,
        click_y,
        canvas_origin_for_test(),
        0.0,
        &layout,
        page_w_px,
        page_h_px,
        page_gap_px,
        zoom,
    );
    let pos = result.expect("zoomed click on page 2 should succeed");
    assert_eq!(
        pos.page_index, 1,
        "should land on page 1 (0-based) at zoom 2×"
    );
}

// ── Reflow (continuous) hit-testing ───────────────────────────────────────

/// One reflow paragraph laid out at the given canvas origin.
fn reflow_para(text: &str, block_index: usize, origin: (f32, f32)) -> PageParagraphData {
    let mut resources = FontResources::new();
    let para = layout_paragraph(
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
            math: None,
            scale: None,
            kerning: None,
            baseline_shift: None,
        }],
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true, // preserve_for_editing — retains the hit-test layout
    );
    PageParagraphData {
        block_index,
        path: Vec::new(),
        layout: Arc::new(para),
        origin,
    }
}

fn two_para_continuous() -> loki_layout::ContinuousLayout {
    let p0 = reflow_para("Hello world", 0, (0.0, 0.0));
    let h0 = p0.layout.height;
    let p1 = reflow_para("Second paragraph here", 1, (0.0, h0));
    loki_layout::ContinuousLayout {
        content_width: 400.0,
        total_height: h0 + p1.layout.height,
        items: vec![],
        paragraphs: vec![p0, p1],
    }
}

#[test]
fn reflow_tap_resolves_to_second_paragraph() {
    let cl = two_para_continuous();
    let h0 = cl.paragraphs[0].layout.height;
    // A tap a couple points into the second paragraph (canvas origin (0,0),
    // no scroll). client coords are CSS px, the band's content inset is
    // REFLOW_PADDING_PT (removed inside the helper).
    let client_y = pt_to_px(h0 + 2.0);
    let client_x = pt_to_px(REFLOW_PADDING_PT + 5.0);
    let (block, _byte) = reflow_hit_test_window(client_x, client_y, (0.0, 0.0), 0.0, &cl)
        .expect("reflow hit lands a position");
    assert_eq!(block, 1, "tap in the second paragraph resolves to block 1");
}

#[test]
fn reflow_tap_in_first_paragraph_resolves_to_block_0() {
    let cl = two_para_continuous();
    let client_y = pt_to_px(2.0); // near the top
    let client_x = pt_to_px(REFLOW_PADDING_PT + 5.0);
    let (block, _) =
        reflow_hit_test_window(client_x, client_y, (0.0, 0.0), 0.0, &cl).expect("reflow hit");
    assert_eq!(block, 0);
}

#[test]
fn reflow_tap_above_canvas_top_is_none() {
    let cl = two_para_continuous();
    // origin.y above the tap ⇒ canvas_y < 0 ⇒ no position.
    assert!(reflow_hit_test_window(10.0, 10.0, (0.0, 100.0), 0.0, &cl).is_none());
}
