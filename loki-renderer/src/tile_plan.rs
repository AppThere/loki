// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Turns a laid-out document plus a viewport into "which tiles mount, at what
//! rasterisation scale" (Spec 08 T2.2).
//!
//! Thin by design: every decision is `appthere_canvas::residency`'s, so the
//! Phase 2 bench and the renderer run the same policy. What lives here is the
//! translation between the renderer's units — CSS-px tile boxes it has already
//! computed for layout — and the model's, which is paper size in points.

use std::sync::Arc;

use appthere_canvas::residency::{PageBox, TextureBudget, ViewportSpec, plan_residency};

use crate::doc_page_source::DocPageSource;

/// CSS pixels per typographic point.
const PTS_TO_CSS_PX: f64 = 96.0 / 72.0;

/// Every page's tile box in CSS px at `zoom`, and whether the layout is a
/// reflow one.
///
/// Both answers come off a single layout guard: taking two would let the
/// generation change between them and pair a reflow flag with paginated boxes.
pub(crate) fn tile_boxes(
    source: &Arc<DocPageSource>,
    generation: u64,
    zoom: f64,
) -> (Vec<(usize, f64, f64)>, bool) {
    let guard = source.layout_for_generation(generation);
    let Some((_, layout)) = guard.as_ref() else {
        return (vec![], false);
    };
    let is_reflow = layout.is_reflow();
    let pages = (0..layout.page_count())
        .filter_map(|i| {
            layout.page_size_pts(i).map(|(w, h)| {
                (
                    i,
                    w as f64 * PTS_TO_CSS_PX * zoom,
                    h as f64 * PTS_TO_CSS_PX * zoom,
                )
            })
        })
        .collect();
    (pages, is_reflow)
}

/// One entry in the render list.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct PlannedTile {
    /// Page index in document order.
    pub(crate) index: usize,
    /// Tile width in CSS px — the on-screen box, unaffected by rasterisation
    /// scale. A reduced-scale tile occupies the same space and is sampled up.
    pub(crate) w: f64,
    /// Tile height in CSS px.
    pub(crate) h: f64,
    /// `Some(scale)` to mount a GPU tile at that rasterisation scale, `None` to
    /// render a blank placeholder.
    pub(crate) mount: Option<f32>,
}

/// Plans the render list for `pages` (page index, width and height in CSS px at
/// the current zoom) under `budget`.
///
/// The CSS boxes are converted back to points because the residency model is
/// keyed on paper size — it has to be, since the physical texture size is
/// `pt × 96/72 × zoom × dsf` and zoom is one of the two axes being bounded.
/// Round-tripping through points rather than threading a second unit through
/// the props keeps one definition of tile size in the tree.
pub(crate) fn plan_tiles(
    pages: &[(usize, f64, f64)],
    gap_px: f64,
    viewport_top_px: f64,
    viewport_height_px: f64,
    zoom: f64,
    device_scale_factor: f64,
    budget: TextureBudget,
) -> Vec<PlannedTile> {
    let zoom = if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    };
    let boxes: Vec<PageBox> = pages
        .iter()
        .map(|&(_, w, h)| PageBox::new(w / (PTS_TO_CSS_PX * zoom), h / (PTS_TO_CSS_PX * zoom)))
        .collect();
    let vp = ViewportSpec {
        scroll_top_px: viewport_top_px,
        client_height_px: viewport_height_px,
        page_gap_px: gap_px,
        zoom,
        device_scale_factor: if device_scale_factor.is_finite() && device_scale_factor > 0.0 {
            device_scale_factor
        } else {
            1.0
        },
    };
    let plan = plan_residency(&boxes, &vp, budget);

    // Runtime observability for the R5a/R28/R29 screen session, and the reason it
    // reports counts rather than only flags: that session's failure mode is
    // passing *without running*. On a 2x display at ordinary zoom the pressure
    // path never engages, so "scrolled around, looked fine" is not evidence about
    // anything. `reduced_tiles` reading non-zero is what makes it evidence.
    let reduced_tiles = plan.tiles.iter().filter(|t| t.raster_scale < 1.0).count();
    if plan.over_budget || reduced_tiles > 0 {
        tracing::debug!(
            budget_bytes = budget.bytes(),
            ceiling_bytes = budget.hard_ceiling_bytes(),
            planned_bytes = plan.total_bytes,
            tiles = plan.tiles.len(),
            reduced_tiles,
            over_budget = plan.over_budget,
            // The distinction L08-026 turns on: over_budget alone is the design
            // working (visible pages full scale, target exceeded and reported),
            // while this means a page the reader is looking at was degraded.
            survival_reduced = plan.survival_reduced,
            "texture residency under pressure",
        );
    }

    pages
        .iter()
        .map(|&(index, w, h)| PlannedTile {
            index,
            w,
            h,
            mount: plan.raster_scale(index),
        })
        .collect()
}

#[cfg(test)]
#[path = "tile_plan_tests.rs"]
mod tests;
