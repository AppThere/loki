// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_layout::PaginatedLayout;

use crate::editing::cursor::DocumentPosition;

use super::{hit_test_page, PX_TO_PT};

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

    hit_test_page(page_index, canvas_x, y_in_page, layout)
}
