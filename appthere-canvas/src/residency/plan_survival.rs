// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Step 5 of `plan_residency` — the survival regime, and the branch beyond it.
//!
//! Extracted from `plan.rs` when that file crossed the 300-line ceiling. The
//! seam is cohesive rather than arbitrary: steps 1–4 all spend *off-centre*
//! work, and everything here is about the visible set, which is the distinction
//! ADR L08-026 turns on.
//!
//! # Two outcomes, and only one of them is a policy
//!
//! Reducing the visible set to fit the survival ceiling is a concession the
//! planner chooses when the alternative is an allocation the device cannot
//! satisfy. Reaching [`MIN_RASTER_SCALE`] and *still* being over the ceiling is
//! not a concession at all — it is the search failing and the plan being mounted
//! regardless. See [`ResidencyPlan::ceiling_exceeded`].

use super::super::geometry::{PageBox, ViewportSpec};
use super::{MIN_RASTER_SCALE, ResidencyPlan, TilePlan, scaled_bytes, total};

/// Reduces the visible set toward `ceiling`, floored at [`MIN_RASTER_SCALE`].
///
/// `visible_bytes` is the set's cost at full scale, already known to exceed
/// `ceiling` — passed in rather than recomputed so the caller's decision and
/// this one cannot disagree about which side of the threshold they are on.
pub(super) fn reduce_to_ceiling(
    mut tiles: Vec<TilePlan>,
    pages: &[PageBox],
    vp: &ViewportSpec,
    cap: u64,
    ceiling: u64,
    visible_bytes: u64,
) -> ResidencyPlan {
    // Bytes go as the square of the scale, so the factor that fits is the square
    // root of the ratio, floored at legibility.
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
    finish_survival(tiles, cap, ceiling)
}

/// Builds the plan for the one path that can leave the visible set above the
/// survival ceiling.
///
/// Separate from `plan::finish` because that function's `ceiling_exceeded: false`
/// is a fact about every earlier return rather than a default: steps 1–4 are at
/// or under the ceiling by construction, so a shared constructor taking the flag
/// as a parameter would invite passing it wrongly from a site where it cannot be
/// true.
fn finish_survival(tiles: Vec<TilePlan>, cap: u64, ceiling: u64) -> ResidencyPlan {
    let total_bytes = total(&tiles);
    ResidencyPlan {
        tiles,
        total_bytes,
        over_target: total_bytes > cap,
        survival_reduced: true,
        ceiling_exceeded: total_bytes > ceiling,
    }
}
