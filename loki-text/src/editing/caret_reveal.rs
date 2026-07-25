// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Caret geometry in **scroll-container content coordinates** (Spec 08 T1.3).
//!
//! `scroll_to_reveal` measures a target rect against
//! [`appthere_ui::ScrollMetrics::visible_rect`], which is in content space —
//! the same space `scroll_top` is in. This module produces the caret's rect in
//! that space, for both renderers.
//!
//! # Where this sits in the transform chain
//!
//! S0.3 documented the full chain (`docs/spikes/S0.3-coordinate-space-audit.md`
//! §1). This module covers steps 2 → 6 and stops short of step 7: it does *not*
//! subtract the scroll offset or add the canvas origin, because a content-space
//! rect is exactly what the reveal wants. `editing::hit_test` walks the same
//! chain in the opposite direction and must stay in agreement with it — the
//! shared constants below are the reason both land on the same pixel.
//!
//! Note the asymmetry S0.3 flags at step 5: `content_items` are content-area
//! local, so page margins must be added, while a paragraph's `origin` is
//! already relative to that content area.

use loki_layout::{ContinuousLayout, PaginatedLayout};

use super::cursor::DocumentPosition;

/// CSS pixels per layout point, before zoom (72 dpi → 96 dpi).
const PT_TO_PX: f32 = 96.0 / 72.0;

/// Nominal caret width in layout points. `CursorRect` carries only `x`, `y` and
/// `height` — a caret is a zero-width line — but the reveal takes a rect, and a
/// zero width would let the horizontal axis treat the caret as already visible
/// when it sits exactly on the right edge.
const CARET_WIDTH_PT: f32 = 1.0;

/// Page stacking geometry, shared by the caret rect and the hit-test.
#[derive(Clone, Copy, Debug)]
pub struct PageStack {
    /// Unscaled page height in CSS px.
    pub page_height_px: f32,
    /// Gap painted between pages, in CSS px. Not scaled by zoom — it is a
    /// fixed CSS margin on the tile, which is why the slot below is
    /// `page × zoom + gap` and not `(page + gap) × zoom`.
    pub page_gap_px: f32,
    /// Zoom fraction (1.0 = 100%).
    pub zoom: f32,
    /// Top padding of the scroll container in CSS px: content y = 0 is the
    /// container's top edge, and the first page starts one padding below it.
    pub content_top_px: f32,
}

impl PageStack {
    /// Vertical distance between the tops of consecutive pages.
    #[must_use]
    pub fn slot_px(&self) -> f32 {
        self.page_height_px * self.zoom + self.page_gap_px
    }

    /// Content-space y for a point `page_local_y_pt` down page `page_index`.
    #[must_use]
    pub fn content_y(&self, page_index: usize, page_local_y_pt: f32) -> f32 {
        self.content_top_px
            + page_index as f32 * self.slot_px()
            + page_local_y_pt * self.px_per_pt()
    }

    /// CSS pixels per layout point at the current zoom.
    #[must_use]
    pub fn px_per_pt(&self) -> f32 {
        PT_TO_PX * self.zoom
    }
}

/// The caret's rect in content coordinates for the **paginated** renderer, as
/// `(x, y, width, height)` in CSS px.
///
/// `None` when the position is not on the current layout — a stale caret after
/// an edit, or a layout that has not been recomputed yet. Callers treat that as
/// "nothing to reveal" rather than scrolling to a guess.
#[must_use]
pub fn caret_rect_paginated(
    layout: &PaginatedLayout,
    pos: &DocumentPosition,
    stack: PageStack,
) -> Option<(f32, f32, f32, f32)> {
    let page = layout.pages.get(pos.page_index)?;
    let editing = page.editing_data.as_ref()?;
    let para = editing
        .paragraphs
        .iter()
        .find(|p| p.block_index == pos.paragraph_index && p.path == pos.path)?;
    let rect = para.layout.cursor_rect(pos.byte_offset)?;

    // Paragraph-local → page-local: the paragraph's origin within the content
    // area, plus the content area's own offset (the page margins).
    let page_x_pt = rect.x + para.origin.0 + page.margins.left;
    let page_y_pt = rect.y + para.origin.1 + page.margins.top;

    let scale = stack.px_per_pt();
    Some((
        page_x_pt * scale,
        stack.content_y(pos.page_index, page_y_pt),
        CARET_WIDTH_PT * scale,
        rect.height * scale,
    ))
}

/// The caret's rect in content coordinates for the **reflow** renderer.
///
/// Reflow bands stack with no gap and the whole flow is one coordinate space,
/// so this is a single scale plus the container padding. `scale` is the reflow
/// type scale, which stands in for zoom in that mode.
#[must_use]
pub fn caret_rect_reflow(
    layout: &ContinuousLayout,
    block_index: usize,
    byte_offset: usize,
    scale: f32,
    content_top_px: f32,
) -> Option<(f32, f32, f32, f32)> {
    let para = layout.paragraph(block_index)?;
    let rect = para.layout.cursor_rect(byte_offset)?;
    let px = PT_TO_PX * scale;
    Some((
        (rect.x + para.origin.0) * px,
        content_top_px + (rect.y + para.origin.1) * px,
        CARET_WIDTH_PT * px,
        rect.height * px,
    ))
}

/// Body line height in CSS px at the caret, used to size the reveal margin.
///
/// Taken from the caret's own line rather than a constant: T1.3 requires the
/// margin to follow the live body style, so three trailing lines is three
/// *actual* lines at the current size and zoom, not 60 px regardless.
/// Falls back to a plausible single-spaced line when the caret has no rect yet.
#[must_use]
pub fn caret_line_height_px(caret_rect: Option<(f32, f32, f32, f32)>, fallback_px: f32) -> f32 {
    match caret_rect {
        Some((_, _, _, h)) if h > 0.5 => h,
        _ => fallback_px,
    }
}

#[cfg(test)]
#[path = "caret_reveal_tests.rs"]
mod tests;
