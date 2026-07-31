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

/// What the planner needs to know about the viewport.
///
/// A struct rather than five positional `f64`s, because the fifth one is where
/// this stopped being readable: `plan_tiles(&pages, 24.0, 2150.0, 24.0, 900.0,
/// 1.0, 2.0, budget)` has two adjacent pixel counts that mean entirely different
/// things, and swapping them compiles. Naming them also stops
/// `content_padding_top_px` — added late, and the whole subject of r67 — from
/// being the easy one to omit at a call site.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewportInput {
    /// The scroll container's raw `scrollTop`, in CSS px.
    pub top_px: f64,
    /// The container's top padding: the distance from `top_px == 0` to the top
    /// of page 0. See
    /// [`crate::view_types::DocumentViewProps::content_padding_top_px`].
    pub content_padding_top_px: f64,
    /// The container's visible height, in CSS px.
    pub height_px: f64,
    /// Current zoom, as a multiplier.
    pub zoom: f64,
    /// The display's device pixel ratio.
    pub device_scale_factor: f64,
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
    viewport: ViewportInput,
    budget: TextureBudget,
) -> Vec<PlannedTile> {
    let ViewportInput {
        top_px: viewport_top_px,
        content_padding_top_px,
        height_px: viewport_height_px,
        zoom,
        device_scale_factor,
    } = viewport;
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
        // Into the model's origin: the model's page 0 starts at 0.0, the
        // container's scroll 0 is one padding above it. Not clamped at 0 — a
        // scroll position inside the padding band is legitimately *negative*
        // in model space, and clamping would put the viewport's top edge on
        // page 0 while the user is still looking at the gap above it.
        scroll_top_px: viewport_top_px - content_padding_top_px,
        client_height_px: viewport_height_px,
        page_gap_px: gap_px,
        zoom,
        device_scale_factor: if device_scale_factor.is_finite() && device_scale_factor > 0.0 {
            device_scale_factor
        } else {
            1.0
        },
    };
    log_budget_change(budget);
    let plan = plan_residency(&boxes, &vp, budget);

    // Runtime observability for the R5a/R28/R29 screen session, and the reason it
    // reports counts rather than only flags: that session's failure mode is
    // passing *without running*. On a 2x display at ordinary zoom the pressure
    // path never engages, so "scrolled around, looked fine" is not evidence about
    // anything. `reduced_tiles` reading non-zero is what makes it evidence.
    //
    // The instrument needs its own control, because a diagnostic reporting zero
    // reductions is indistinguishable from zero reductions occurring — which is
    // the exact ambiguity it exists to remove (L9-011). So the screen procedure
    // is two settings, not one: force `LOKI_TEXTURE_BUDGET_MB` absurdly low and
    // confirm this line reports a non-zero `reduced_tiles` *first*, establishing
    // that it can speak, and only then set the value under test and trust it when
    // it says nothing. See Spec 08 §Phase 2's closing procedure.
    let reduced_tiles = plan.tiles.iter().filter(|t| t.raster_scale < 1.0).count();
    if plan.ceiling_exceeded {
        // Its own line, at warn, because it is the only outcome this planner
        // produces that it did not choose: the scale search hit its floor and
        // the plan was mounted over the OOM-calibrated ceiling regardless. A
        // debug line alongside the ordinary pressure reporting would put it in
        // a stream people filter out, which is how it stayed silent until r30.
        tracing::warn!(
            ceiling_bytes = budget.hard_ceiling_bytes(),
            planned_bytes = plan.total_bytes,
            over_by_bytes = plan.total_bytes.saturating_sub(budget.hard_ceiling_bytes()),
            tiles = plan.tiles.len(),
            "texture residency ABOVE the survival ceiling — page too large to \
             serve at this zoom on this device, mounting anyway",
        );
    }
    if plan.over_target || reduced_tiles > 0 {
        tracing::debug!(
            budget_bytes = budget.bytes(),
            ceiling_bytes = budget.hard_ceiling_bytes(),
            planned_bytes = plan.total_bytes,
            tiles = plan.tiles.len(),
            reduced_tiles,
            over_target = plan.over_target,
            // The distinction L08-026 turns on: over_target alone is the design
            // working (visible pages full scale, target exceeded and reported),
            // while this means a page the reader is looking at was degraded.
            survival_reduced = plan.survival_reduced,
            ceiling_exceeded = plan.ceiling_exceeded,
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

/// Reports the budget in force whenever it changes, including the first time it
/// is observed.
///
/// # Why the pressure line above is not enough
///
/// That line carries `budget_bytes`, but it is only emitted *under pressure* — so
/// a run that prints nothing tells you neither what the budget was nor that the
/// budget was large enough to avoid pressure. Those are the two readings the
/// screen procedure has to distinguish, and the control it already has (force the
/// budget absurdly low, confirm the line can speak) only establishes the
/// instrument works; it says nothing about the value in force on the *next* run,
/// which is the run under test.
///
/// This is the same defect one level up from the one the pressure line's own
/// comment describes (L9-011): a diagnostic that is silent in the success case
/// cannot be told from a diagnostic that is broken. So the resolved budget is
/// announced unconditionally, and a quiet run becomes evidence rather than an
/// absence of it.
///
/// Logged on change rather than once per process because the budget is reactive:
/// it follows the device profile, so it moves when the DPR probe lands or RAM
/// availability is re-read. Once-per-process would report a pre-probe value and
/// then never correct it.
fn log_budget_change(budget: TextureBudget) {
    use std::sync::atomic::{AtomicU64, Ordering};

    // `u64::MAX` is the "nothing observed yet" sentinel: it is not a reachable
    // budget, so the first call always differs and always logs.
    static LAST_BYTES: AtomicU64 = AtomicU64::new(u64::MAX);
    static LAST_CEILING: AtomicU64 = AtomicU64::new(u64::MAX);

    let bytes = budget.bytes();
    let ceiling = budget.hard_ceiling_bytes();
    // Separate statements, so both swaps run whatever the first compares to —
    // `||` would short-circuit and leave the ceiling's record stale.
    let bytes_changed = LAST_BYTES.swap(bytes, Ordering::Relaxed) != bytes;
    let ceiling_changed = LAST_CEILING.swap(ceiling, Ordering::Relaxed) != ceiling;
    if bytes_changed || ceiling_changed {
        tracing::debug!(
            budget_bytes = bytes,
            ceiling_bytes = ceiling,
            // Which arm of the derivation produced it — the difference between
            // "this machine was measured" and "the override or the baseline
            // stood in for it", which the byte figure alone does not show.
            source = ?budget.source(),
            "texture budget in force",
        );
    }
}

#[cfg(test)]
#[path = "tile_plan_tests.rs"]
mod tests;
