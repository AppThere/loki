// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Canvas-coordinate to document-position hit testing.
//!
//! # Coordinate-transform strategy
//!
//! **Strategy C — calculated from known layout values.**
//!
//! Neither `use_mounted` / `MountedData::get_client_rect()` nor
//! `offset_x` / `offset_y` are available in dioxus-native-dom 0.7.4
//! (both are `unimplemented!()`). Pointer events provide only
//! `ClientPoint` (window-relative logical pixels). The canvas origin
//! is therefore computed from:
//!
//! - `canvas_origin.x` = `(window_inner_width_px − page_width_px) / 2.0`
//!   (pages are flex-centered; `window_inner_width_px` defaults to 1280 px
//!   and must be updated when a window-size API becomes available in Blitz).
//! - `canvas_origin.y` = `TOOLBAR_HEIGHT_TOP + SPACE_6` (exact from tokens).
//!
//! - `scroll_offset` = 0.0 (Blitz does not expose `node.scroll_offset` to
//!   Dioxus components; wired as a TODO once the API is available).
//!
//! All geometry inside this function works in layout **points** (1 pt = 1/72 in).
//! The conversion from CSS logical pixels is applied once at entry:
//! `pt = px × (72/96)`.

use loki_layout::PaginatedLayout;

use super::cursor::DocumentPosition;

/// CSS pixels → layout points scale factor (72 dpi / 96 dpi).
const PX_TO_PT: f32 = 72.0 / 96.0;

/// Translates a window-relative pointer position into a [`DocumentPosition`]
/// using the paginated layout's editing data.
///
/// Returns `None` when:
/// - the click is outside all page content areas,
/// - `preserve_for_editing` was `false` (no editing data available),
/// - no paragraph could be found at the hit point, or
/// - the paragraph's Parley layout was not retained (read-only mode).
///
/// # Parameters
///
/// * `client_x` / `client_y` — window-relative pointer position in CSS
///   logical pixels (from `MouseEvent::client_coordinates()`).
/// * `canvas_origin` — the top-left of the first-page canvas in window
///   coordinates (CSS logical pixels). Computed via Strategy C.
/// * `scroll_offset` — current vertical scroll position of the document
///   scroll container in CSS pixels. Pass `0.0` until Blitz exposes scroll
///   offset to Dioxus components.
/// * `layout` — paginated layout produced with
///   `LayoutOptions { preserve_for_editing: true }`.
/// * `page_width_px` / `page_height_px` — page canvas dimensions in CSS px.
/// * `page_gap_px` — vertical gap between page canvases in CSS px.
// All arguments are semantically distinct; grouping into a struct would reduce clarity at call sites.
#[allow(clippy::too_many_arguments)]
pub fn hit_test_document(
    client_x: f32,
    client_y: f32,
    canvas_origin: (f32, f32),
    scroll_offset: f32,
    layout: &PaginatedLayout,
    _page_width_px: f32,
    page_height_px: f32,
    page_gap_px: f32,
) -> Option<DocumentPosition> {
    // ── 1. Canvas-local coordinates in CSS pixels ─────────────────────────────
    let canvas_x_px = client_x - canvas_origin.0;
    let canvas_y_px = client_y - canvas_origin.1 + scroll_offset;

    if canvas_x_px < 0.0 {
        return None;
    }

    // ── 2. Convert to layout points (1 pt = 96/72 CSS px) ────────────────────
    let canvas_x = canvas_x_px * PX_TO_PT;
    let canvas_y = canvas_y_px * PX_TO_PT;

    let page_height_pt = page_height_px * PX_TO_PT;
    let page_gap_pt = page_gap_px * PX_TO_PT;

    // ── 3. Determine which page was clicked ───────────────────────────────────
    let page_and_gap = page_height_pt + page_gap_pt;
    if page_and_gap <= 0.0 {
        return None;
    }
    let page_index = (canvas_y / page_and_gap) as usize;
    let y_in_page = canvas_y - (page_index as f32 * page_and_gap);

    // Reject clicks in the gap between pages.
    if y_in_page > page_height_pt || y_in_page < 0.0 {
        return None;
    }

    hit_test_page(
        page_index,
        canvas_x,
        y_in_page,
        layout,
    )
}

/// Hit-test a single page using coordinates already relative to the page's
/// content area top-left (in layout points).
///
/// This is used by the per-page event handlers to bypass window-relative
/// origin and scroll offset calculations.
pub fn hit_test_page(
    page_index: usize,
    canvas_x: f32, // in layout points, relative to page left
    canvas_y: f32, // in layout points, relative to page top
    layout: &PaginatedLayout,
) -> Option<DocumentPosition> {
    let page = layout.pages.get(page_index)?;
    let editing_data = page.editing_data.as_ref()?;

    // ── 4. Page-content-area-local coordinates ────────────────────────────────
    // page.margins is in layout points; margins.left/top are already in pt.
    let content_x = canvas_x - page.margins.left;
    let content_y = canvas_y - page.margins.top;

    // ── 5. Identify the paragraph under the click ─────────────────────────────
    // paragraphs are content-area-local (x, y) in layout points.
    let para_data = editing_data.paragraphs.iter().rev().find(|p| {
        p.origin.1 <= content_y && content_y <= p.origin.1 + p.layout.height
    })?;

    // ── 6. Map to byte offset within the paragraph ────────────────────────────
    let x_in_para = content_x - para_data.origin.0;
    let y_in_para = content_y - para_data.origin.1;

    let hit = para_data.layout.hit_test_point(x_in_para, y_in_para)?;

    Some(DocumentPosition {
        page_index,
        paragraph_index: para_data.block_index,
        byte_offset: hit.byte_offset,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loki_layout::{
        layout_paragraph, FontResources, LayoutInsets, LayoutPage, LayoutSize,
        PaginatedLayout, PageEditingData, PageParagraphData, ResolvedParaProps, StyleSpan,
        LayoutColor,
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
        let margins = LayoutInsets { top: 72.0, right: 72.0, bottom: 72.0, left: 72.0 };
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
        PaginatedLayout { page_size, pages: vec![page] }
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
            margin_left_px,          // client_x = canvas_x = margin_left in px
            margin_top_px,           // client_y = canvas_y = margin_top in px
            canvas_origin_for_test(),
            0.0,                     // scroll_offset
            &layout,
            page_w_px,
            page_h_px,
            pt_to_px(24.0),          // page_gap_px
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
            page_h_px + 100.0,       // in the inter-page gap
            canvas_origin_for_test(),
            0.0,
            &layout,
            page_w_px,
            page_h_px,
            pt_to_px(24.0),
        );
        assert!(result.is_none(), "click below page content area must return None");
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
            PaginatedLayout { page_size: single.page_size, pages: vec![page0, page1] }
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
        assert_eq!(pos.page_index, 1, "scroll-adjusted click must land on page 1 (0-based)");
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
            PaginatedLayout { page_size: single.page_size, pages: vec![page0, page1] }
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
            assert_ne!(pos.page_index, 1, "without scroll_offset, click must not reach page 1");
        }
    }
}
