// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What to mount, and at what rasterisation scale, to stay inside the budget
//! (Spec 08 T2.2, ADR L08-002).
//!
//! # The order of concessions
//!
//! L08-002 fixes it: **budget pressure reduces rasterisation scale rather than
//! evicting visible content.** Concretely, and in this order:
//!
//! 1. Everything in the virtualization window at full scale. If that fits,
//!    nothing else happens — the common case at ordinary zoom.
//! 2. Step the **off-centre** tiles — inside the window, outside the visible
//!    rect — down a fixed scale ladder. These are the pre-render margin; the
//!    user is not looking at them, and a tile that scrolls into view re-renders
//!    at full scale.
//! 3. Drop off-centre tiles, furthest from the viewport first. They become
//!    blank placeholders, exactly as an un-windowed page already is.
//! 4. Only if the **visible** tiles alone still exceed the budget, reduce
//!    *their* scale — to the exact factor that fits, not a ladder step, and
//!    never below [`MIN_RASTER_SCALE`]. This is the 400%-on-a-3×-display corner.
//!
//! Visible tiles are never dropped, at any budget. That is the acceptance
//! criterion, and it is why step 4 exists at all: without it a small budget
//! would have no legal move left and the only remaining lever would be
//! eviction.
//!
//! # Why a ladder for off-centre and an exact factor for visible
//!
//! Off-centre tiles change identity constantly as the user scrolls. A
//! continuous scale would re-render them on every scroll event, since the
//! rasterisation scale is part of the tile's invalidation key — the saving
//! would be paid for in GPU work. A five-step ladder means a tile re-renders
//! only when it crosses a step.
//!
//! The visible set does not have that problem: its byte cost is fixed by zoom
//! and DPI, not by scroll position, so an exact factor is stable and wastes no
//! resolution.

use super::budget::TextureBudget;
use super::geometry::{PageBox, ViewportSpec, visible_window};

/// Rasterisation scales an off-centre tile may take, in order of preference.
pub const RASTER_SCALE_LADDER: &[f32] = &[1.0, 0.75, 0.5, 0.35, 0.25];

/// The lowest rasterisation scale any tile may take.
///
/// A page at 0.25 is rendered at a quarter of its linear device resolution —
/// at 400% zoom that is still an effective 100%, soft but legible. Below this
/// the degradation stops reading as "not settled yet" and starts reading as
/// broken, which is the failure R5 tracks.
pub const MIN_RASTER_SCALE: f32 = 0.25;

/// One tile in a residency plan.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TilePlan {
    /// Page index in document order.
    pub page_index: usize,
    /// Rasterisation scale to allocate this tile's texture at, in `(0, 1]`.
    pub raster_scale: f32,
    /// Requested texture bytes at that scale.
    pub bytes: u64,
    /// Whether the page overlaps the visible rect, as opposed to the grown
    /// window. Visible tiles are never dropped.
    pub visible: bool,
}

/// The mounting decision for one viewport state.
#[derive(Clone, PartialEq, Debug)]
pub struct ResidencyPlan {
    /// Tiles to mount, in document order.
    pub tiles: Vec<TilePlan>,
    /// Requested texture bytes across [`Self::tiles`].
    pub total_bytes: u64,
    /// `true` when the plan could not reach the budget without breaking a rule
    /// it will not break — i.e. the visible set exceeds the budget even at
    /// [`MIN_RASTER_SCALE`].
    ///
    /// Reported rather than resolved. The alternatives are dropping a page the
    /// user is looking at or rendering it below legibility, and both are worse
    /// than being over budget on a device that cannot do better.
    pub over_budget: bool,
}

impl ResidencyPlan {
    /// The scale for `page_index`, or `None` when the page is not mounted.
    #[must_use]
    pub fn raster_scale(&self, page_index: usize) -> Option<f32> {
        self.tiles
            .iter()
            .find(|t| t.page_index == page_index)
            .map(|t| t.raster_scale)
    }
}

/// Which pages overlap the visible rect itself, rather than the grown window.
#[must_use]
pub fn strictly_visible(pages: &[PageBox], vp: &ViewportSpec) -> Vec<bool> {
    let mut page_top = 0.0_f64;
    let vis_lo = vp.scroll_top_px;
    let vis_hi = vp.scroll_top_px + vp.client_height_px.max(1.0);
    pages
        .iter()
        .map(|p| {
            let h = p.css_size(vp.zoom).1;
            let overlaps = (page_top + h) >= vis_lo && page_top <= vis_hi;
            page_top += h + vp.page_gap_px;
            overlaps
        })
        .collect()
}

/// Plans the resident set for one viewport state under `budget`.
#[must_use]
pub fn plan_residency(
    pages: &[PageBox],
    vp: &ViewportSpec,
    budget: TextureBudget,
) -> ResidencyPlan {
    let heights: Vec<f64> = pages.iter().map(|p| p.css_size(vp.zoom).1).collect();
    let windowed = visible_window(
        &heights,
        vp.page_gap_px,
        vp.scroll_top_px,
        vp.client_height_px,
    );
    let visible = strictly_visible(pages, vp);

    let mut tiles: Vec<TilePlan> = pages
        .iter()
        .enumerate()
        .filter(|(i, _)| windowed.get(*i).copied().unwrap_or(false))
        .map(|(i, page)| TilePlan {
            page_index: i,
            raster_scale: 1.0,
            bytes: scaled_bytes(*page, vp, 1.0),
            visible: visible.get(i).copied().unwrap_or(false),
        })
        .collect();

    let cap = budget.bytes();
    if total(&tiles) <= cap {
        return finish(tiles, false);
    }

    // Step 2: walk the ladder for off-centre tiles.
    for &scale in RASTER_SCALE_LADDER.iter().skip(1) {
        for tile in tiles.iter_mut().filter(|t| !t.visible) {
            tile.raster_scale = scale;
            tile.bytes = scaled_bytes(pages[tile.page_index], vp, scale);
        }
        if total(&tiles) <= cap {
            return finish(tiles, false);
        }
    }

    // Step 3: drop off-centre tiles, furthest from the viewport first.
    let centre = viewport_centre_page(&heights, vp);
    while total(&tiles) > cap {
        let Some(pos) = tiles
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.visible)
            .max_by_key(|(_, t)| t.page_index.abs_diff(centre))
            .map(|(pos, _)| pos)
        else {
            break;
        };
        tiles.remove(pos);
    }
    if total(&tiles) <= cap {
        return finish(tiles, false);
    }

    // Step 4: the visible set alone is over budget. Reduce its scale to the
    // exact factor that fits — bytes go as the square of the scale, so the
    // factor is the square root of the ratio — floored at legibility.
    let visible_bytes = total(&tiles);
    if visible_bytes == 0 {
        return finish(tiles, false);
    }
    let mut scale =
        ((cap as f64 / visible_bytes as f64).sqrt() as f32).clamp(MIN_RASTER_SCALE, 1.0);
    loop {
        for tile in &mut tiles {
            tile.raster_scale = scale;
            tile.bytes = scaled_bytes(pages[tile.page_index], vp, scale);
        }
        if total(&tiles) <= cap || scale <= MIN_RASTER_SCALE {
            break;
        }
        // The closed form is slightly optimistic: each axis rounds *up* to whole
        // device pixels, so the realised area can exceed the ideal by a fraction
        // of a row and column. Stepping down settles it in a few iterations and
        // is bounded by the legibility floor — worth more than a closed form
        // that is right on paper and over budget in practice.
        scale = (scale * 0.98).max(MIN_RASTER_SCALE);
    }
    let over = total(&tiles) > cap;
    finish(tiles, over)
}

fn finish(tiles: Vec<TilePlan>, over_budget: bool) -> ResidencyPlan {
    let total_bytes = total(&tiles);
    ResidencyPlan {
        tiles,
        total_bytes,
        over_budget,
    }
}

fn total(tiles: &[TilePlan]) -> u64 {
    tiles.iter().map(|t| t.bytes).sum()
}

fn scaled_bytes(page: PageBox, vp: &ViewportSpec, scale: f32) -> u64 {
    page.texture_bytes(vp.zoom, vp.device_scale_factor * f64::from(scale))
}

/// Index of the page under the middle of the viewport — the distance origin for
/// step 3's eviction order.
fn viewport_centre_page(heights: &[f64], vp: &ViewportSpec) -> usize {
    let mid = vp.scroll_top_px + vp.client_height_px / 2.0;
    let mut top = 0.0_f64;
    for (i, h) in heights.iter().enumerate() {
        if mid <= top + h {
            return i;
        }
        top += h + vp.page_gap_px;
    }
    heights.len().saturating_sub(1)
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
