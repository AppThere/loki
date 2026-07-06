// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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
//! - `canvas_origin.x` = `(viewport_width_px − page_width_px) / 2.0` (pages are
//!   flex-centered). `viewport_width_px` is the **measured** scroll-container
//!   width (`scroll_metrics.client_width`), wrapped in
//!   [`crate::editing::viewport::Viewport`] which owns this centring math — see
//!   `Viewport::centred_origin_x`. (Previously a hardcoded 1280 px default;
//!   Spec 01 audit A-1.)
//! - `canvas_origin.y` = `TOOLBAR_HEIGHT_TOP + SPACE_6` (exact from tokens).
//!
//! - `scroll_offset` = the live scroll position mirrored from the canvas
//!   `onscroll` handler (`editor_canvas.rs` → `scroll_metrics`), threaded in
//!   by the pointer handlers (`editor_pointer.rs`).
//!
//! All geometry inside this function works in layout **points** (1 pt = 1/72 in).
//! The conversion from CSS logical pixels is applied once at entry:
//! `pt = px × (72/96)`.

use loki_layout::{ContinuousLayout, PaginatedLayout};
use loki_renderer::render_layout::REFLOW_PADDING_PT;

use super::cursor::DocumentPosition;

/// CSS pixels → layout points scale factor (72 dpi / 96 dpi).
const PX_TO_PT: f32 = 72.0 / 96.0;

/// Translates a window-relative pointer position into a continuous-layout
/// document position `(block_index, byte_offset)` for **reflow** mode.
///
/// The reflow canvas is one continuous vertical flow (no pages); the content is
/// inset by [`REFLOW_PADDING_PT`] within each band tile, so that inset is
/// removed here to match the layout's paragraph origins. `canvas_origin` and
/// `scroll_offset` follow the same Strategy-C model as [`hit_test_document`]:
/// the vertical origin (`TOOLBAR_HEIGHT_TOP + SPACE_6`) is exact, so the hit
/// always lands on the correct line; the horizontal origin is approximate
/// (Blitz exposes no element rect), affecting only which character within the
/// line is chosen — and [`ContinuousLayout::hit_test`] clamps that.
///
/// `layout` must be the same width as the painted reflow layout (build it from
/// `scroll_metrics().client_width`), or line breaks diverge from what is drawn.
pub fn reflow_hit_test_window(
    client_x: f32,
    client_y: f32,
    canvas_origin: (f32, f32),
    scroll_offset: f32,
    layout: &ContinuousLayout,
) -> Option<(usize, usize)> {
    let canvas_x_px = client_x - canvas_origin.0;
    let canvas_y_px = client_y - canvas_origin.1 + scroll_offset;
    if canvas_y_px < 0.0 {
        return None;
    }
    let canvas_x = canvas_x_px * PX_TO_PT - REFLOW_PADDING_PT;
    let canvas_y = canvas_y_px * PX_TO_PT;
    layout.hit_test(canvas_x, canvas_y)
}

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
/// * `page_width_px` / `page_height_px` — **unzoomed** page canvas dimensions in
///   CSS px (as stored in `DocumentState`). The painted tiles are these scaled
///   by `zoom`; this function reapplies the scale internally.
/// * `page_gap_px` — vertical gap between page canvases in CSS px. This is a
///   fixed CSS margin that is **not** scaled by zoom (see
///   `loki_renderer::document_view`).
/// * `zoom` — the paginated render zoom (1.0 = 1:1). The caller must also pass a
///   `canvas_origin.0` centred on the **scaled** page width
///   (`centred_origin_x(page_width_px * zoom)`), since the tiles are centred at
///   their painted width.
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
    zoom: f32,
) -> Option<DocumentPosition> {
    let zoom = if zoom > 0.0 { zoom } else { 1.0 };

    // ── 1. Canvas-local coordinates in CSS pixels ─────────────────────────────
    let canvas_x_px = client_x - canvas_origin.0;
    let canvas_y_px = client_y - canvas_origin.1 + scroll_offset;

    if canvas_x_px < 0.0 {
        return None;
    }

    // ── 2. Locate the page in scaled CSS px ──────────────────────────────────
    // Tiles are painted at `zoom` scale; the inter-page gap is a fixed CSS
    // margin that is not scaled. Work the page stride in CSS px, then convert
    // the in-page offset to layout points, dividing the zoom back out.
    let page_h_scaled = page_height_px * zoom;
    let slot_px = page_h_scaled + page_gap_px;
    if slot_px <= 0.0 {
        return None;
    }
    let page_index = (canvas_y_px / slot_px) as usize;
    let y_in_page_px = canvas_y_px - (page_index as f32 * slot_px);

    // Reject clicks in the gap between pages.
    if y_in_page_px > page_h_scaled || y_in_page_px < 0.0 {
        return None;
    }

    // ── 3. In-page CSS px → layout points (1 pt = 96/72 CSS px, ÷ zoom) ───────
    let px_to_pt = PX_TO_PT / zoom;
    let canvas_x = canvas_x_px * px_to_pt;
    let y_in_page = y_in_page_px * px_to_pt;

    hit_test_page(page_index, canvas_x, y_in_page, layout)
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

    // Clicks above the page top are outside the canvas — no valid position.
    if canvas_y < 0.0 {
        return None;
    }

    // ── 4. Page-content-area-local coordinates ────────────────────────────────
    // page.margins is in layout points; margins.left/top are already in pt.
    let content_x = canvas_x - page.margins.left;
    let content_y = canvas_y - page.margins.top;

    // ── 5. Identify the paragraph under the click ─────────────────────────────
    // paragraphs are content-area-local (x, y) in layout points.
    //
    // Primary: find the paragraph whose vertical extent covers the click.
    // Fallback A: click above all content → first paragraph.
    // Fallback B: click below all content → last paragraph.
    // Both fallbacks are needed so that clicking anywhere on a blank-document
    // page (which has a single zero-height empty paragraph) places the cursor.
    let para_data = editing_data
        .paragraphs
        .iter()
        .rev()
        .find(|p| p.origin.1 <= content_y && content_y <= p.origin.1 + p.layout.height)
        .or_else(|| {
            // Prefer the last paragraph for clicks below all content; it covers
            // both the "below last line" and the "empty document" cases.
            editing_data.paragraphs.last()
        })?;

    // ── 6. Map to byte offset within the paragraph ────────────────────────────
    let x_in_para = content_x - para_data.origin.0;
    let y_in_para = (content_y - para_data.origin.1).max(0.0);

    // hit_test_point returns None only when preserve_for_editing is false.
    // In editing mode it always returns Some; fall back to offset 0 defensively.
    let byte_offset = para_data
        .layout
        .hit_test_point(x_in_para, y_in_para)
        .map_or(0, |h| h.byte_offset);

    Some(DocumentPosition {
        page_index,
        paragraph_index: para_data.block_index,
        byte_offset,
        // Carry the nesting descent so a click inside a table cell / note body
        // resolves to the right nested paragraph (empty for top-level).
        path: para_data.path.clone(),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "hit_test_tests.rs"]
mod tests;
