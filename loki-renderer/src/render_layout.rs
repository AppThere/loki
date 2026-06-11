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

use loki_layout::{ContinuousLayout, PaginatedLayout};
use loki_vello::{CursorPaint, FontDataCache};

/// Height of one reflow render tile in layout points (~1365 CSS px).  Tall
/// enough to keep tile counts low, short enough that a Hot-tier texture
/// (2× scale) stays well inside common GPU texture limits.
pub const REFLOW_TILE_HEIGHT_PT: f32 = 1024.0;

/// Horizontal reading-margin inset in points (24 CSS px each side).  The
/// reflow layout is computed at `tile width − 2 × inset` and painted shifted
/// right by this amount, so text never touches the viewport edge.
pub const REFLOW_PADDING_PT: f32 = 18.0;

/// Narrowest reflow content width the layout engine is asked for, in points.
/// Guards against degenerate layouts while the viewport width is still being
/// measured.
pub const MIN_REFLOW_CONTENT_PT: f32 = 50.0;

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
    Paginated(PaginatedLayout),
    /// Continuous reflow layout — sliced into virtual tiles of
    /// [`REFLOW_TILE_HEIGHT_PT`].  The stored width is the full tile width in
    /// points (content width plus both padding insets).
    Reflow(ContinuousLayout),
}

impl RenderLayout {
    /// Borrow the paginated layout, or `None` in reflow mode.  Callers that
    /// need per-page editing data (cursor painting, hit testing) use this and
    /// degrade gracefully in reflow mode, which carries no editing data.
    pub fn as_paginated(&self) -> Option<&PaginatedLayout> {
        match self {
            RenderLayout::Paginated(pl) => Some(pl),
            RenderLayout::Reflow(_) => None,
        }
    }

    /// `true` when this is a reflow (zero-gap virtual tile) layout.
    pub fn is_reflow(&self) -> bool {
        matches!(self, RenderLayout::Reflow(_))
    }

    /// Number of render tiles (pages, or reflow bands).
    pub fn page_count(&self) -> usize {
        match self {
            RenderLayout::Paginated(pl) => pl.pages.len(),
            RenderLayout::Reflow(cl) => {
                (cl.total_height / REFLOW_TILE_HEIGHT_PT).ceil().max(1.0) as usize
            }
        }
    }

    /// Size of one render tile in layout points, or `None` if out of range.
    pub fn page_size_pts(&self, index: usize) -> Option<(f32, f32)> {
        match self {
            RenderLayout::Paginated(pl) => pl
                .pages
                .get(index)
                .map(|p| (p.page_size.width, p.page_size.height)),
            RenderLayout::Reflow(cl) => {
                if index >= self.page_count() {
                    return None;
                }
                let width = cl.content_width + 2.0 * REFLOW_PADDING_PT;
                let band_top = index as f32 * REFLOW_TILE_HEIGHT_PT;
                let height = (cl.total_height - band_top).clamp(1.0, REFLOW_TILE_HEIGHT_PT);
                Some((width, height))
            }
        }
    }

    /// Paint render tile `index` into `scene` at `scale`.
    ///
    /// Paginated tiles paint the full page (chrome, shadow, optional cursor);
    /// reflow tiles paint the band of the continuous flow they cover, inset by
    /// [`REFLOW_PADDING_PT`].  The reflow band has no explicit background — the
    /// caller's `RenderParams::base_color` (white) provides it.  `cursor_paint`
    /// is ignored in reflow mode, which carries no editing data.
    pub fn paint_tile(
        &self,
        scene: &mut vello::Scene,
        font_cache: &mut FontDataCache,
        index: usize,
        scale: f32,
        cursor_paint: Option<&CursorPaint>,
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
            RenderLayout::Reflow(cl) => {
                let band_top = index as f32 * REFLOW_TILE_HEIGHT_PT;
                let band_h = self
                    .page_size_pts(index)
                    .map(|(_, h)| h)
                    .unwrap_or(REFLOW_TILE_HEIGHT_PT);
                loki_vello::paint_continuous_band(
                    scene,
                    cl,
                    font_cache,
                    REFLOW_PADDING_PT,
                    scale,
                    band_top,
                    band_h,
                );
            }
        }
    }
}
