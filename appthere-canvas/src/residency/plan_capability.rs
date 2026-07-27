// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The highest zoom this device can actually serve for a given page size
//! (Spec 08 §3.6f — input for Phase 5 T5.4, not wired here).
//!
//! # Why this is a query and not a sixth step
//!
//! `plan_residency` has one outcome it did not choose: the scale search reaches
//! [`super::MIN_RASTER_SCALE`] and the visible set is still over the survival
//! ceiling, so the plan is mounted above the threshold calibrated against the OOM
//! killer. On A0 at 2 GiB that is ~2.5 GiB requested on a machine with 2 GiB
//! available — which does not degrade, it dies. Warning is the right thing to do
//! about a state you are in; it is not a way to be in a survivable state.
//!
//! Adding a branch to handle it would be a third compensation for whole-page
//! granularity, and ADR L08-030 says exactly what to do with those: **further
//! refinement of the budget policy is work sub-page tiling deletes.** So the
//! answer is not another step. It is to never enter the regime, by bounding zoom
//! at what the device can serve rather than at the constant
//! [`super::MAX_ZOOM`].
//!
//! That is honest in the way a branch is not — the limit is real, so it belongs
//! in the control — and it costs the planner nothing, because this function is
//! not on the planning path at all.
//!
//! # The check that made a clamp the right answer rather than a hopeful one
//!
//! A clamp only helps if the unservable regime has a *lower* edge inside the zoom
//! range. If a page exceeded the ceiling at [`super::MIN_ZOOM`], no zoom limit would
//! reach it and the honest interim would be a load-time refusal instead.
//!
//! Measured — the zoom at which each exceeding row first crosses the ceiling:
//!
//! | page | available | display | survival from | exceeds from |
//! | --- | ---: | ---: | ---: | ---: |
//! | A2 | 2 GiB | 4x | 100% | 325% |
//! | A1 | 2 GiB | 3x | 75% | 300% |
//! | A1 | 2 GiB | 4x | 75% | 225% |
//! | A0 | 2 GiB | 3x | 75% | 225% |
//! | A0 | 2 GiB | 4x | 50% | **175%** |
//! | A0 | 8-16 GiB | 4x | 100% | 325% |
//!
//! **No row exceeds at minimum zoom**, and the lowest edge is 175%. So a
//! capability clamp closes every case, and the load-time-refusal branch is not
//! needed. The cost is real and worth stating plainly: an A0 document on a 2 GiB
//! 4x device would be limited to about 150% zoom.

use super::budget::TextureBudget;
use super::geometry::{
    MAX_ZOOM_PERMILLE, MIN_ZOOM, MIN_ZOOM_PERMILLE, PageBox, ViewportSpec, zoom_from_permille,
};
use super::plan::plan_residency;

/// Zoom granularity this search reports at, in thousandths — 5%.
///
/// Finer than any control is likely to offer, so the answer is limited by the
/// device rather than by this constant.
pub const ZOOM_PROBE_STEP_PERMILLE: u16 = 50;

/// Scroll offsets sampled per candidate zoom.
///
/// The visible set is one page at most offsets and two where a page boundary
/// crosses the viewport, and it is the two-page offsets that peak. A single
/// offset would over-report the servable zoom by roughly a factor of √2 — an
/// answer that is wrong in the dangerous direction.
const OFFSET_SAMPLES: u32 = 40;

/// The largest zoom at which `page` can be mounted without exceeding `budget`'s
/// survival ceiling, on a display of `device_scale_factor`. **In thousandths.**
///
/// Returns [`MAX_ZOOM_PERMILLE`] when the device can serve the whole range,
/// which is the case for A4 and Letter on every memory size measured. Never
/// returns below [`MIN_ZOOM_PERMILLE`]: if even minimum zoom were unservable a
/// clamp would be the wrong instrument, and the caller needs to distinguish that
/// — see [`is_servable_at_all`].
///
/// # Thousandths, not a factor
///
/// This value is a **bound a caller compares against**, which makes it a
/// predicate hazard in floating point. The first draft returned `f64` and
/// accumulated `zoom += 0.05`, landing on 3.999999999999994 — unservable by
/// 4e-15, which would have clamped every device on ordinary paper. Stepping by
/// integer index fixed that instance; returning an integer removes the class,
/// including the one still ahead: a user typing exactly the limit in a Phase 5
/// zoom control must land *on* it.
///
/// # Not wired, and what T5.4 must do with it
///
/// Nothing calls this yet. Phase 5's T5.4 rewrites the zoom control and is where
/// the clamp belongs — landing it there makes it a feature of the control rather
/// than a branch in the policy.
///
/// **T5.4 must call this rather than derive its own bound.** The property test
/// `clamping_to_the_servable_zoom_makes_the_oom_branch_unreachable` guards a
/// function nobody calls; if T5.4 limits zoom from first principles, that test
/// stays green while production still exceeds the ceiling. That is L08-029's
/// two-derivations failure, and it is cheap to avoid: `DocPageSource::set_zoom`
/// is the single clamp site in the tree, so wiring is one call there.
///
/// **The limit is live, and must not act retroactively.** The memory probe
/// re-samples every 5 s, the ceiling derives from available RAM, and this
/// derives from the ceiling — so a user sitting at 300% can have the limit fall
/// below them when another process takes memory. Forcing a zoom-out mid-edit is
/// worse than never having offered 300%, and reads as the app malfunctioning.
/// The rule T5.4 should implement: **this gates new zoom requests and never
/// reduces a zoom already in effect.** A session may sit above the current limit
/// — it was servable when entered, `ceiling_exceeded` reports it if it stops
/// being so, and the alternative is worse.
#[must_use]
pub fn max_servable_zoom_permille(
    page: PageBox,
    device_scale_factor: f64,
    budget: TextureBudget,
) -> u16 {
    // Searched against the real planner rather than solved in closed form. The
    // closed form needs the visible-page count, the per-axis whole-pixel
    // rounding and the floor's interaction with the ceiling — all of which are
    // already decided in `plan_residency`, and a second derivation of them is a
    // second thing that can disagree with production. Same reason
    // `visible_window` is the production mounting rule rather than a model of it.
    let doc = [page; 8];
    let mut best = MIN_ZOOM_PERMILLE;
    let mut permille = MIN_ZOOM_PERMILLE;
    while permille <= MAX_ZOOM_PERMILLE {
        if !servable(
            &doc,
            zoom_from_permille(permille),
            device_scale_factor,
            budget,
        ) {
            // Demand is monotonic in zoom, so the first failure is the edge.
            break;
        }
        best = permille;
        permille = permille.saturating_add(ZOOM_PROBE_STEP_PERMILLE);
    }
    best
}

/// Whether `page` can be served at *any* zoom on this device.
///
/// `false` means a zoom clamp is the wrong instrument — the document does not
/// fit on this device at all, and the honest response is to say so at load time
/// rather than to offer a range in which every value is unservable. No measured
/// combination returns `false` today; it is here so a caller has to handle the
/// case rather than assume it away.
#[must_use]
pub fn is_servable_at_all(page: PageBox, device_scale_factor: f64, budget: TextureBudget) -> bool {
    servable(&[page; 8], MIN_ZOOM, device_scale_factor, budget)
}

fn servable(doc: &[PageBox], zoom: f64, dsf: f64, budget: TextureBudget) -> bool {
    let page_h = doc.first().map_or(1.0, |p| p.css_size(zoom).1);
    // Offsets spread over a few page pitches, so boundary-in-viewport offsets are
    // sampled whatever the page height at this zoom.
    let pitch = (page_h + 24.0).max(1.0);
    for i in 0..OFFSET_SAMPLES {
        let top = pitch * 2.0 + pitch * f64::from(i) / f64::from(OFFSET_SAMPLES) * 3.0;
        let vp = ViewportSpec::new(top, 900.0, zoom, dsf);
        if plan_residency(doc, &vp, budget).ceiling_exceeded {
            return false;
        }
    }
    true
}

#[cfg(test)]
#[path = "plan_capability_tests.rs"]
mod tests;
