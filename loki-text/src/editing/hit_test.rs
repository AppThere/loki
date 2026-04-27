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

use loki_layout::{PaginatedLayout, HitTestResult};

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
pub fn hit_test_document(
    client_x: f32,
    client_y: f32,
    canvas_origin: (f32, f32),
    scroll_offset: f32,
    layout: &PaginatedLayout,
    page_width_px: f32,
    page_height_px: f32,
    page_gap_px: f32,
) -> Option<DocumentPosition> {
    // ── 1. Canvas-local coordinates in CSS pixels ─────────────────────────────
    let canvas_x_px = client_x - canvas_origin.0;
    let canvas_y_px = client_y - canvas_origin.1 + scroll_offset;

    // Reject clicks outside the page canvas horizontally.
    if canvas_x_px < 0.0 || canvas_x_px > page_width_px {
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
    if y_in_page > page_height_pt {
        return None;
    }

    let page = layout.pages.get(page_index)?;
    let editing_data = page.editing_data.as_ref()?;

    // ── 4. Page-content-area-local coordinates ────────────────────────────────
    // page.margins is in layout points; margins.left/top are already in pt.
    let content_x = canvas_x - page.margins.left;
    let content_y = y_in_page - page.margins.top;

    // ── 5. Identify the paragraph under the click ─────────────────────────────
    // paragraph_origins are content-area-local (x, y) in layout points.
    // Find the last paragraph whose origin.y ≤ content_y.
    let para_index = editing_data
        .paragraph_origins
        .iter()
        .enumerate()
        .rev()
        .find(|(_, origin)| origin.1 <= content_y)
        .map(|(i, _)| i)?;

    // ── 6. Paragraph-local coordinates ───────────────────────────────────────
    let origin = editing_data.paragraph_origins[para_index];
    let para_x = content_x - origin.0;
    let para_y = content_y - origin.1;

    // ── 7. Hit test against the Parley layout ─────────────────────────────────
    let para_layout = editing_data.paragraph_layouts.get(para_index)?.as_ref()?;
    let HitTestResult { byte_offset, .. } = para_layout.hit_test_point(para_x, para_y)?;

    Some(DocumentPosition { page_index, paragraph_index: para_index, byte_offset })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loki_layout::{
        layout_paragraph, FontResources, LayoutInsets, LayoutOptions, LayoutPage, LayoutSize,
        PaginatedLayout, PageEditingData, ResolvedParaProps, StyleSpan, LayoutColor,
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
        let para_height = para.height;
        let editing_data = PageEditingData {
            paragraph_layouts: vec![Some(Arc::new(para))],
            paragraph_origins: vec![(0.0, 0.0)],
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
        let _ = para_height; // used above; suppress warning
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
}
