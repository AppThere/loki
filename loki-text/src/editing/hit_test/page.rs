// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_layout::PaginatedLayout;

use crate::editing::cursor::DocumentPosition;

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
    })
}

#[cfg(test)]
mod tests;
