// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! *How soft* does the survival regime get? (Spec 08 R5b, R29.)
//!
//! `plan_reachability_tests` answers whether step 5 fires. This answers what it
//! does when it does, which is the question R5b's outcome actually turns on: if
//! reduction could walk down unbounded, "how soft" would have no answer short of
//! unusable and would depend on where the ladder happened to stop on the day.
//!
//! # There is a floor, it is [`MIN_RASTER_SCALE`], and it binds
//!
//! Step 5 solves for the exact factor that fits the ceiling — bytes go as the
//! square of scale, so that is `sqrt(ceiling / demand)` — and clamps it at
//! `MIN_RASTER_SCALE` (0.25). The loop then **breaks at the floor and mounts
//! anyway**, over the ceiling. So the floor is not a limit on how far the search
//! goes; it is a standing decision that *exceeding the survival ceiling is
//! preferable to going softer than a quarter linear resolution*.
//!
//! That is worth stating plainly because it is the only place in this planner
//! where the ceiling — the thing calibrated against the OOM killer — is
//! knowingly exceeded.
//!
//! # Measured depth, and what raising the floor would cost
//!
//! Worst visible scale over the whole clamped zoom range and 40 scroll offsets,
//! with the ceiling exceedance a raised floor would incur (demand at the raised
//! floor over the ceiling, so `1.0x` means the raise is free at that point):
//!
//! | page | available | display | needed scale | @0.50 | @0.75 | @0.85 |
//! | --- | ---: | ---: | ---: | ---: | ---: | ---: |
//! | Letter | 2 GiB | 2x | 0.764 | 1.0x | 1.0x | 1.2x |
//! | Letter | 2 GiB | 3x | 0.510 | 1.0x | 2.2x | 2.8x |
//! | Letter | 2 GiB | 4x | 0.382 | 1.7x | 3.9x | 4.9x |
//! | Letter | 4 GiB | 4x | 0.552 | 1.0x | 1.8x | 2.4x |
//! | Letter | 8-16 GiB | 4x | 0.780 | 1.0x | 1.0x | 1.2x |
//! | A3 | 2 GiB | 2x | 0.531 | 1.0x | 2.0x | 2.6x |
//! | A3 | 2 GiB | 4x | **0.271** | 3.4x | 7.7x | **9.8x** |
//! | A3 | 4 GiB | 3x | 0.501 | 1.0x | 2.2x | 2.9x |
//! | A3 | 8-16 GiB | 3x | 0.709 | 1.0x | 1.1x | 1.4x |
//! | A3 | 8-16 GiB | 4x | 0.542 | 1.0x | 1.9x | 2.5x |
//!
//! Two readings, and they point opposite ways:
//!
//! - **The floor is not decorative.** A3 on a 2 GiB machine at 4x needs 0.271 —
//!   within 0.021 of the floor. The extreme is already at the bound.
//! - **A high floor is not affordable *there*, and is nearly free everywhere
//!   else.** 0.85 costs 9.8x the ceiling at that point — ~2.5 GiB requested on a
//!   machine with 2 GiB available, which is the OOM the ceiling exists to avoid.
//!   But on every 8 GiB-or-more row it costs 1.2-1.4x, and 0.75 costs 1.0-1.1x.
//!
//! So "raise the floor" is not one decision. The steepness is concentrated on
//! low-memory, high-DPI, large-paper combinations — where the honest answer may
//! be that whole-page tiles cannot serve that device at that zoom at all, which
//! is I-22's case rather than a policy knob's.
//!
//! These tests pin the depth; they do not choose the floor. Changing
//! `MIN_RASTER_SCALE` is a decision about what a reader may be shown, and it
//! should fail these and be re-measured rather than pass quietly.

use super::super::budget::{BudgetInputs, TextureBudget};
use super::super::geometry::{PageBox, ViewportSpec, ZOOM_RANGE_MAX, ZOOM_RANGE_MIN};
use super::{MIN_RASTER_SCALE, plan_residency};

fn a3() -> PageBox {
    PageBox::new(842.0, 1191.0)
}

fn budget_for(available_gib: f64) -> TextureBudget {
    TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some((available_gib * 1024.0 * 1024.0 * 1024.0) as u64),
        total_ram_bytes: None,
        gpu_paint_path: Some(true),
        user_override_bytes: None,
    })
}

/// Lowest scale any *visible* tile is mounted at, over the clamped zoom range
/// and 40 scroll offsets. `1.0` means the survival regime never engaged.
fn worst_visible_scale(page: PageBox, available_gib: f64, dsf: f64) -> f32 {
    let doc = vec![page; 500];
    let budget = budget_for(available_gib);
    let mut worst = 1.0_f32;
    let mut zoom = ZOOM_RANGE_MIN;
    while zoom <= ZOOM_RANGE_MAX + f64::EPSILON {
        for offset in 0..40 {
            let vp = ViewportSpec::new(20_000.0 + f64::from(offset) * 500.0, 900.0, zoom, dsf);
            let plan = plan_residency(&doc, &vp, budget);
            for tile in plan.tiles.iter().filter(|t| t.visible) {
                worst = worst.min(tile.raster_scale);
            }
        }
        zoom += 0.25;
    }
    worst
}

/// Softness is bounded by construction, not by where the search happened to
/// stop. Without this the answer to "how soft does it get" is "unusable", and
/// R5b would be judging an unbounded quantity.
#[test]
fn no_visible_tile_is_ever_mounted_below_the_floor() {
    for page in [PageBox::us_letter(), a3()] {
        for gib in [2.0_f64, 4.0, 8.0, 16.0, 64.0] {
            for dsf in [1.0_f64, 2.0, 3.0, 4.0] {
                let worst = worst_visible_scale(page, gib, dsf);
                assert!(
                    worst >= MIN_RASTER_SCALE,
                    "a visible tile reached {worst} at {gib} GiB / {dsf}x, below the \
                     {MIN_RASTER_SCALE} floor — softness is unbounded",
                );
            }
        }
    }
}

/// The floor binds in practice, so it is a live policy rather than a
/// theoretical guard. The extreme sits within 0.03 of it.
///
/// If this stops holding, the floor has become decorative and the reachability
/// picture that R5b rests on has moved.
#[test]
fn the_extreme_operating_point_sits_against_the_floor() {
    let worst = worst_visible_scale(a3(), 2.0, 4.0);
    assert!(
        (MIN_RASTER_SCALE..MIN_RASTER_SCALE + 0.03).contains(&worst),
        "A3 on 2 GiB at 4x reached {worst}; it was measured at 0.271, hard against \
         the {MIN_RASTER_SCALE} floor. Outside that band the demand curve or the \
         ceiling changed and the depth table in these docs needs re-measuring",
    );
}

/// The number that belongs on screen for R5b's own judgement: what a reader is
/// actually shown at an ordinary large-machine operating point, as opposed to
/// the worst case a spec table can find.
///
/// 0.709 is a materially different thing to look at from 0.271 — one is a page
/// at 71% linear resolution, the other at a quarter — and R5b answered against
/// the wrong one would be answered wrongly.
#[test]
fn an_ordinary_large_machine_is_reduced_to_about_three_quarters() {
    let worst = worst_visible_scale(a3(), 16.0, 3.0);
    assert!(
        (0.68..0.75).contains(&worst),
        "A3 on 16 GiB at 3x reached {worst}, outside the 0.68-0.75 band it was \
         measured at — this is the figure R5b is judged on, so a change here \
         changes what the screen session is looking at",
    );
}

/// A 1x display never reaches the regime on any memory size, so the whole
/// question is HiDPI's. Recorded because it bounds who is affected.
#[test]
fn a_standard_dpi_display_never_enters_the_regime() {
    for gib in [2.0_f64, 4.0, 8.0, 16.0, 64.0] {
        for page in [PageBox::us_letter(), a3()] {
            assert_eq!(
                worst_visible_scale(page, gib, 1.0),
                1.0,
                "a 1x display at {gib} GiB entered the survival regime; the claim \
                 that this is a HiDPI-only concern no longer holds",
            );
        }
    }
}
