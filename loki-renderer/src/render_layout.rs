// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Render-side layout representation: paginated pages or reflow tiles.
//!
//! [`RenderLayout`] is what [`crate::DocPageSource`] caches and what the tile
//! components consume.  In paginated mode it wraps the print-fidelity
//! [`PaginatedLayout`] directly.  In reflow mode it wraps a
//! [`ContinuousLayout`] (the real layout engine run at the viewport width — see
//! `LayoutMode::Reflow`) and presents it as a sequence of fixed-height virtual
//! tiles so the existing page cache, scroll tiering, and texture machinery
//! work unchanged.  Tiles are stacked with zero gap; items spanning a tile
//! boundary are painted into both tiles (see `loki_vello::band`).

use std::sync::Arc;

use loki_layout::{ContinuousLayout, CursorRect, PaginatedLayout};
use loki_vello::{CursorPaint, FontDataCache, SelectionRect};

/// Height of one reflow render tile in layout points. Chosen so the CSS-pixel
/// height is an exact integer (768 pt × 96/72 = 1024 px — fractional heights
/// leave a sub-pixel seam between stacked tiles), tall enough to keep tile
/// counts low, short enough for a 2×-scale texture to fit GPU limits.
pub const REFLOW_TILE_HEIGHT_PT: f32 = 768.0;

/// Horizontal reading-margin inset in points (24 CSS px each side): reflow is
/// computed at `tile width − 2 × inset` and painted shifted right by it.
pub const REFLOW_PADDING_PT: f32 = 18.0;

/// Narrowest reflow content width the layout engine is asked for, in points.
/// Guards against degenerate layouts while the viewport width is still being
/// measured.
pub const MIN_REFLOW_CONTENT_PT: f32 = 50.0;

/// Widest reflow **tile** (CSS px) — caps the reading measure so the
/// non-paginated view holds a comfortable line length and **centres** on wide
/// windows (Spec 03 M4 / D5); below this the tile tracks the viewport. Matches
/// the HTML reflow fallback's `max-width` so both paths read identically.
pub const MAX_REFLOW_TILE_PX: f32 = 820.0;

/// CSS pixels → layout points (72 dpi / 96 dpi).
pub const PX_TO_PT: f32 = 72.0 / 96.0;

/// The reflow **tile** width (CSS px) for a measured viewport: the viewport
/// width capped at [`MAX_REFLOW_TILE_PX`] (the renderer centres the tile, so
/// the cap centres the reading column). The single source of reflow width for
/// paint, hit-testing, and keyboard navigation (Spec 01 A-1).
#[must_use]
pub fn reflow_tile_width_px(viewport_width_px: f32) -> f32 {
    viewport_width_px.clamp(0.0, MAX_REFLOW_TILE_PX)
}

/// The reflow **content** width (the reading measure, in points): the tile
/// minus the [`REFLOW_PADDING_PT`] side insets, floored at
/// [`MIN_REFLOW_CONTENT_PT`]. Hit-testing and navigation match the paint.
#[must_use]
pub fn reflow_content_width_pt(viewport_width_px: f32) -> f32 {
    (reflow_tile_width_px(viewport_width_px) * PX_TO_PT - 2.0 * REFLOW_PADDING_PT)
        .max(MIN_REFLOW_CONTENT_PT)
}

// ── RenderMode ────────────────────────────────────────────────────────────────

/// Which layout the renderer should produce.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode {
    /// Print-fidelity pages from the document's own page geometry.
    Paginated,
    /// Continuous web-style flow at the given available width (layout points,
    /// including the [`REFLOW_PADDING_PT`] insets on both sides).
    Reflow {
        /// Total tile width in points.
        available_width_pt: f32,
    },
}

impl RenderMode {
    /// Width-tolerant equality: reflow widths within half a point are treated
    /// as the same mode so sub-pixel viewport jitter does not trigger relayout.
    pub(crate) fn matches(&self, other: &RenderMode) -> bool {
        match (self, other) {
            (RenderMode::Paginated, RenderMode::Paginated) => true,
            (
                RenderMode::Reflow {
                    available_width_pt: a,
                },
                RenderMode::Reflow {
                    available_width_pt: b,
                },
            ) => (a - b).abs() < 0.5,
            _ => false,
        }
    }
}

// ── RenderLayout ──────────────────────────────────────────────────────────────

/// A computed layout in either rendering shape.
#[derive(Debug, Clone)]
pub enum RenderLayout {
    /// Paginated print layout — one tile per real page.
    ///
    /// Held behind an [`Arc`] so the editor's already-computed layout can be
    /// shared with the renderer without a deep clone (the single canonical
    /// layout — see [`crate::DocPageSource::provide_paginated_layout`]).
    Paginated(Arc<PaginatedLayout>),
    /// Continuous reflow layout — sliced into virtual tiles of
    /// [`REFLOW_TILE_HEIGHT_PT`].
    Reflow {
        /// The continuous layout (items in absolute canvas coordinates).
        layout: ContinuousLayout,
        /// Full tile width in points: `max(wrap width, widest content) + both
        /// padding insets`.  Wider than the viewport when content overflows
        /// (e.g. a fixed-width table), so it can be reached by horizontal
        /// scrolling instead of being clipped.
        tile_width_pt: f32,
    },
}

impl RenderLayout {
    /// Borrow the paginated layout, or `None` in reflow mode.  Callers that
    /// need per-page editing data (cursor painting, hit testing) use this and
    /// degrade gracefully in reflow mode, which carries no editing data.
    pub fn as_paginated(&self) -> Option<&PaginatedLayout> {
        match self {
            RenderLayout::Paginated(pl) => Some(pl.as_ref()),
            RenderLayout::Reflow { .. } => None,
        }
    }

    /// `true` when this is a reflow (zero-gap virtual tile) layout.
    pub fn is_reflow(&self) -> bool {
        matches!(self, RenderLayout::Reflow { .. })
    }

    /// Number of render tiles (pages, or reflow bands).
    pub fn page_count(&self) -> usize {
        match self {
            RenderLayout::Paginated(pl) => pl.pages.len(),
            RenderLayout::Reflow { layout, .. } => (layout.total_height / REFLOW_TILE_HEIGHT_PT)
                .ceil()
                .max(1.0) as usize,
        }
    }

    /// Size of one render tile in layout points, or `None` if out of range.
    pub fn page_size_pts(&self, index: usize) -> Option<(f32, f32)> {
        match self {
            RenderLayout::Paginated(pl) => pl
                .pages
                .get(index)
                .map(|p| (p.page_size.width, p.page_size.height)),
            RenderLayout::Reflow {
                layout,
                tile_width_pt,
            } => {
                if index >= self.page_count() {
                    return None;
                }
                let band_top = index as f32 * REFLOW_TILE_HEIGHT_PT;
                let height = (layout.total_height - band_top).clamp(1.0, REFLOW_TILE_HEIGHT_PT);
                Some((*tile_width_pt, height))
            }
        }
    }

    /// The continuous layout, or `None` in paginated mode.
    fn continuous(&self) -> Option<&ContinuousLayout> {
        match self {
            RenderLayout::Reflow { layout, .. } => Some(layout),
            RenderLayout::Paginated(_) => None,
        }
    }

    /// Hit-test a point in **canvas** coordinates (layout points, padding
    /// already removed) against the reflow layout, returning
    /// `(block_index, byte_offset)`.  `None` in paginated mode.
    pub fn reflow_hit_test(&self, canvas_x: f32, canvas_y: f32) -> Option<(usize, usize)> {
        self.continuous()?.hit_test(canvas_x, canvas_y)
    }

    /// Hyperlink URL under a point in **canvas** coordinates, or `None` in
    /// paginated mode / over plain text (feature 5.11, reflow half).
    pub fn reflow_link_at(&self, canvas_x: f32, canvas_y: f32) -> Option<String> {
        self.continuous()?
            .link_at(canvas_x, canvas_y)
            .map(str::to_owned)
    }

    /// Caret rectangle in **canvas** coordinates for `(block_index,
    /// byte_offset)`, or `None` in paginated mode / when not found.
    pub(crate) fn reflow_cursor_canvas(
        &self,
        block_index: usize,
        byte_offset: usize,
    ) -> Option<CursorRect> {
        self.continuous()?
            .cursor_rect_canvas(block_index, byte_offset)
    }

    /// Paint render tile `index` into `scene` at `scale`.
    ///
    /// Paginated tiles paint the full page (chrome, shadow, optional cursor via
    /// `cursor_paint`).  Reflow tiles paint the band of the continuous flow they
    /// cover, inset by [`REFLOW_PADDING_PT`]; the reflow caret is painted from
    /// `reflow_cursor` = `(block_index, byte_offset)` when it falls in the band.
    /// The reflow band has no explicit background — the caller's
    /// `RenderParams::base_color` (white) provides it.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_tile(
        &self,
        scene: &mut vello::Scene,
        font_cache: &mut FontDataCache,
        index: usize,
        scale: f32,
        cursor_paint: Option<&CursorPaint>,
        reflow_cursor: Option<(usize, usize)>,
        reflow_selection: Option<((usize, usize), (usize, usize))>,
    ) {
        match self {
            RenderLayout::Paginated(pl) => {
                loki_vello::paint_single_page(
                    scene,
                    pl,
                    font_cache,
                    (0.0, 0.0),
                    scale,
                    index,
                    cursor_paint,
                );
            }
            RenderLayout::Reflow { layout, .. } => {
                let band_top = index as f32 * REFLOW_TILE_HEIGHT_PT;
                let band_h = self
                    .page_size_pts(index)
                    .map(|(_, h)| h)
                    .unwrap_or(REFLOW_TILE_HEIGHT_PT);
                let offset = (REFLOW_PADDING_PT, -band_top);

                // Selection highlight first, so glyphs and the caret sit on top.
                // Clip to rects that intersect this band; paint_cursor draws the
                // selection rects (cursor_rect height 0 ⇒ no caret here).
                if let Some((a, b)) = reflow_selection {
                    let sel: Vec<SelectionRect> = layout
                        .selection_rects(a, b)
                        .into_iter()
                        .filter(|r| {
                            r.origin.y + r.size.height >= band_top
                                && r.origin.y <= band_top + band_h
                        })
                        .map(|r| SelectionRect {
                            x: r.origin.x,
                            y: r.origin.y,
                            width: r.size.width,
                            height: r.size.height,
                        })
                        .collect();
                    if !sel.is_empty() {
                        loki_vello::paint_cursor(
                            scene,
                            &CursorRect {
                                x: 0.0,
                                y: 0.0,
                                height: 0.0,
                            },
                            &sel,
                            &[],
                            offset,
                            scale,
                        );
                    }
                }

                loki_vello::paint_continuous_band(
                    scene,
                    layout,
                    font_cache,
                    REFLOW_PADDING_PT,
                    scale,
                    band_top,
                    band_h,
                );

                // Caret: paint when its canvas rect intersects this band,
                // translated into band-local space (same offset as the items).
                if let Some((para, byte)) = reflow_cursor
                    && let Some(cr) = self.reflow_cursor_canvas(para, byte)
                    && cr.y + cr.height >= band_top
                    && cr.y <= band_top + band_h
                {
                    loki_vello::paint_cursor(scene, &cr, &[], &[], offset, scale);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "render_layout_tests.rs"]
mod tests;
