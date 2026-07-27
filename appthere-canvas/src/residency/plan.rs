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
//! 4. If the **visible** tiles alone still exceed the budget, **stop**. Mount
//!    them at full scale and report [`ResidencyPlan::over_target`]. The budget is
//!    a target, and it has run out of things it is allowed to spend.
//! 5. Only above the **survival ceiling** — `TextureBudget::hard_ceiling_bytes`,
//!    a threshold far above the target — reduce the visible set's scale, to the
//!    exact factor that fits and never below [`MIN_RASTER_SCALE`].
//!
//! Visible tiles are never dropped, at any budget or ceiling.
//!
//! # Why step 5 is not step 4 (ADR L08-026)
//!
//! Until r15 there was no step 5: the visible set was reduced as soon as it
//! exceeded the *target*. That looked like a corner case and was not one. With
//! the real device scale factor wired (R27), the visible set exceeds a derived
//! target during ordinary reading: at 200% zoom on an 8 GiB machine with a 2×
//! display it does so at **40% of scroll offsets** — every offset where a page
//! boundary sits inside the viewport — so body text would soften and re-sharpen
//! as the reader scrolls. At 200% on a 3× display it is 100% of offsets at 0.51
//! scale, and even a 16 GiB desktop softens on a 3× display.
//!
//! That is not a memory policy, it is a rendering defect. The byte target is
//! ours to choose; the reader's perception is not. So the target may only ever
//! buy memory back from work the user cannot see, and the one case where
//! degrading visible text is the right answer is the case where the alternative
//! is the process being killed — which is what the survival ceiling names.
//!
//! **Why not "decline to mount" instead?** It was considered and rejected: for
//! *visible* content, refusing to mount is a blank page where the user is
//! reading, which is strictly worse than a soft one and contradicts the
//! never-drop-visible criterion. Declining is the honest answer for off-centre
//! tiles, and that is exactly what step 3 already does.
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
///
/// Since r15 this floor binds in two very different places: on off-centre tiles,
/// where it is never seen at rest (R5a), and on the visible set in the survival
/// regime only (R5b), where it is seen but the alternative is an allocation the
/// device cannot satisfy.
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
    /// `true` when the plan exceeds the byte **target** after spending
    /// everything the target is allowed to spend.
    ///
    /// Reported rather than resolved, and since r15 this is an ordinary outcome
    /// rather than a corner: the visible set at full scale can exceed a derived
    /// target during normal reading on a HiDPI machine, and the correct response
    /// is to say so, not to soften the page. See the module docs for why.
    pub over_target: bool,
    /// `true` when the visible set's scale was reduced to stay under the
    /// **survival ceiling** — the only circumstance in which this planner
    /// degrades what the user is looking at.
    ///
    /// Distinct from [`Self::over_target`] because they carry opposite meanings
    /// for a reader of a diagnostic: over-target is the design working, while
    /// this is the device having run out of room.
    pub survival_reduced: bool,
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
        return finish(tiles, false, false);
    }

    // Step 2: walk the ladder for off-centre tiles.
    for &scale in RASTER_SCALE_LADDER.iter().skip(1) {
        for tile in tiles.iter_mut().filter(|t| !t.visible) {
            tile.raster_scale = scale;
            tile.bytes = scaled_bytes(pages[tile.page_index], vp, scale);
        }
        if total(&tiles) <= cap {
            return finish(tiles, false, false);
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
        return finish(tiles, false, false);
    }

    // Step 4: the visible set alone is over target. That is where the target's
    // authority ends — it may not spend legibility (L08-026). Mount full scale
    // and report it.
    let visible_bytes = total(&tiles);
    let ceiling = budget.hard_ceiling_bytes();
    if visible_bytes == 0 || visible_bytes <= ceiling {
        return finish(tiles, true, false);
    }

    // Step 5: above the survival ceiling there is no benign move left — refusing
    // to degrade means requesting an allocation the device cannot satisfy. Reduce
    // to the exact factor that fits *the ceiling*; bytes go as the square of the
    // scale, so the factor is the square root of the ratio, floored at legibility.
    let mut scale =
        ((ceiling as f64 / visible_bytes as f64).sqrt() as f32).clamp(MIN_RASTER_SCALE, 1.0);
    loop {
        for tile in &mut tiles {
            tile.raster_scale = scale;
            tile.bytes = scaled_bytes(pages[tile.page_index], vp, scale);
        }
        if total(&tiles) <= ceiling || scale <= MIN_RASTER_SCALE {
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
    finish(tiles, over, true)
}

fn finish(tiles: Vec<TilePlan>, over_target: bool, survival_reduced: bool) -> ResidencyPlan {
    let total_bytes = total(&tiles);
    ResidencyPlan {
        tiles,
        total_bytes,
        over_target,
        survival_reduced,
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

#[cfg(test)]
#[path = "plan_reachability_tests.rs"]
mod reachability_tests;
