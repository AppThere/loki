// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Selection-handle grab geometry (mobile drag-to-adjust).
//!
//! The renderer paints teardrop handles below the selection edges (paginated
//! mode, Android). To let the user drag them, a touch start must be matched
//! against the handles' on-screen positions — this module maps a selection
//! endpoint back to its window-relative grab point (the inverse of
//! [`hit_test_document`]'s transform) and decides whether a touch grabs a
//! handle.
//!
//! [`hit_test_document`]: super::hit_test::hit_test_document

use loki_layout::PaginatedLayout;

use super::cursor::DocumentPosition;

/// Radius (CSS px) around a handle's grab point within which a touch starts a
/// handle drag. Generous — a fingertip target, not a stylus one.
pub const HANDLE_GRAB_RADIUS_PX: f32 = 32.0;

/// Vertical distance (layout points) from the selection edge's line bottom to
/// the teardrop circle centre. Mirrors the painter's geometry in
/// `loki-vello/src/scene_cursor.rs` (`HANDLE_STEM_HEIGHT` 24 + `HANDLE_CIRCLE_RADIUS` 8).
const HANDLE_GRAB_OFFSET_PT: f32 = 32.0;

/// CSS pixels per layout point (before zoom).
const PT_TO_PX: f32 = 96.0 / 72.0;

/// The window-relative (client CSS px) grab point of the selection handle for
/// the endpoint `pos`: the teardrop circle centre below that caret's line.
///
/// `canvas_origin`, `scroll_offset`, `page_height_px`, `page_gap_px` and
/// `zoom` are the same values the forward hit-test uses, so grab points land
/// exactly where the handles are painted. Returns `None` when the position is
/// not on the layout (stale) or sits in a table cell (no handles painted
/// there).
#[allow(clippy::too_many_arguments)]
pub fn handle_grab_point(
    layout: &PaginatedLayout,
    pos: &DocumentPosition,
    canvas_origin: (f32, f32),
    scroll_offset: f32,
    page_height_px: f32,
    page_gap_px: f32,
    zoom: f32,
) -> Option<(f32, f32)> {
    if !pos.path.is_empty() {
        return None; // table-cell selections paint no handles
    }
    let zoom = if zoom > 0.0 { zoom } else { 1.0 };
    let page = layout.pages.get(pos.page_index)?;
    let editing_data = page.editing_data.as_ref()?;
    let para = editing_data
        .paragraphs
        .iter()
        .find(|p| p.block_index == pos.paragraph_index && p.path.is_empty())?;
    let rect = para.layout.cursor_rect(pos.byte_offset)?;

    // Paragraph-local → page-local (content origin + margins), in points.
    let page_x_pt = rect.x + para.origin.0 + page.margins.left;
    let page_y_pt = rect.y + rect.height + para.origin.1 + page.margins.top + HANDLE_GRAB_OFFSET_PT;

    // Page-local points → canvas CSS px (pages stacked with an unscaled gap),
    // then canvas → client. Exact inverse of `hit_test_document`.
    let pt_to_px = PT_TO_PX * zoom;
    let slot_px = page_height_px * zoom + page_gap_px;
    let canvas_x_px = page_x_pt * pt_to_px;
    let canvas_y_px = pos.page_index as f32 * slot_px + page_y_pt * pt_to_px;
    Some((
        canvas_x_px + canvas_origin.0,
        canvas_y_px + canvas_origin.1 - scroll_offset,
    ))
}

/// If `touch` (client CSS px) grabs one of the selection's two handles,
/// returns the **other** endpoint — the one that stays fixed. The caller
/// normalises the selection to `anchor = fixed`, so the drag always moves the
/// focus, and the handles may freely cross.
#[allow(clippy::too_many_arguments)]
pub fn grab_fixed_endpoint(
    layout: &PaginatedLayout,
    anchor: &DocumentPosition,
    focus: &DocumentPosition,
    touch: (f32, f32),
    canvas_origin: (f32, f32),
    scroll_offset: f32,
    page_height_px: f32,
    page_gap_px: f32,
    zoom: f32,
) -> Option<DocumentPosition> {
    let near = |p: &DocumentPosition| {
        handle_grab_point(
            layout,
            p,
            canvas_origin,
            scroll_offset,
            page_height_px,
            page_gap_px,
            zoom,
        )
        .is_some_and(|(gx, gy)| {
            let (dx, dy) = (touch.0 - gx, touch.1 - gy);
            dx.hypot(dy) <= HANDLE_GRAB_RADIUS_PX
        })
    };
    if near(anchor) {
        return Some(focus.clone());
    }
    if near(focus) {
        return Some(anchor.clone());
    }
    None
}

#[cfg(test)]
#[path = "selection_handles_tests.rs"]
mod tests;
