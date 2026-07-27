// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The property Phase 5 will depend on: clamping to
//! [`max_servable_zoom_permille`] makes `ceiling_exceeded` unreachable.
//!
//! Stated as an implication rather than as a table of numbers, because the
//! numbers are the device's and the implication is the contract.
//!
//! # This guards a function nobody calls
//!
//! Said plainly, because it is the limitation that matters: nothing in
//! production calls `max_servable_zoom_permille` yet. If T5.4 implements zoom
//! limiting from first principles — or with its own constant — every test here
//! stays green while production still exceeds the ceiling. That is exactly
//! L08-029's two-derivations failure.
//!
//! So the requirement is recorded in T5.4's text and on the function itself:
//! **call it, do not re-derive it.** `DocPageSource::set_zoom` is the single
//! clamp site in the tree, so the wiring is one call.

use super::super::budget::{BudgetInputs, TextureBudget};
use super::super::geometry::{
    PageBox, ViewportSpec, ZOOM_RANGE_MAX_PERMILLE, ZOOM_RANGE_MIN_PERMILLE, zoom_from_permille,
};
use super::super::plan::plan_residency;
use super::{
    ZOOM_PROBE_STEP_PERMILLE, is_servable_at_all, max_full_scale_zoom_permille,
    max_servable_zoom_permille,
};

fn iso_a(n: usize) -> PageBox {
    let sizes = [
        (595.28, 841.89),
        (841.89, 1190.55),
        (1190.55, 1683.78),
        (1683.78, 2383.94),
        (2383.94, 3370.39),
    ];
    let (w, h) = sizes[4 - n.min(4)];
    PageBox::new(w, h)
}

fn budget_for(available_gib: f64) -> TextureBudget {
    TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some((available_gib * 1024.0 * 1024.0 * 1024.0) as u64),
        total_ram_bytes: None,
        gpu_paint_path: Some(true),
        user_override_bytes: None,
    })
}

/// The contract: at or below the reported zoom, the OOM branch cannot be
/// entered — at any scroll offset, on any page size, on any device.
///
/// This is the whole justification for preferring a clamp to a sixth planner
/// step. If it does not hold, the clamp is a partial mitigation dressed as a
/// solution and the branch still needs handling.
#[test]
fn clamping_to_the_servable_zoom_makes_the_oom_branch_unreachable() {
    for n in 0..=4_usize {
        for gib in [2.0_f64, 4.0, 8.0, 16.0, 64.0] {
            for dsf in [1.0_f64, 2.0, 3.0, 4.0] {
                let page = iso_a(n);
                let budget = budget_for(gib);
                let limit = max_servable_zoom_permille(page, dsf, budget);
                let doc = [page; 8];
                let mut permille = ZOOM_RANGE_MIN_PERMILLE;
                while permille <= limit {
                    let zoom = zoom_from_permille(permille);
                    let pitch = doc[0].css_size(zoom).1 + 24.0;
                    for i in 0..40 {
                        let top = pitch * 2.0 + pitch * f64::from(i) / 40.0 * 3.0;
                        let vp = ViewportSpec::new(top, 900.0, zoom, dsf);
                        assert!(
                            !plan_residency(&doc, &vp, budget).ceiling_exceeded,
                            "A{n} at {gib} GiB / {dsf}x exceeded the ceiling at \
                             {permille} permille, at or below the reported servable \
                             limit {limit}",
                        );
                    }
                    permille += ZOOM_PROBE_STEP_PERMILLE;
                }
            }
        }
    }
}

/// The clamp must not be free by being vacuous: on the sizes that reach the OOM
/// branch it has to actually bite, and on ordinary paper it must not.
#[test]
fn the_clamp_bites_only_where_the_device_cannot_serve() {
    // A4 and A3 are what real documents use; a limit below ZOOM_RANGE_MAX there would
    // be a regression dressed as a safety feature.
    for n in [4_usize, 3] {
        for gib in [2.0_f64, 4.0, 8.0, 16.0, 64.0] {
            for dsf in [1.0_f64, 2.0, 3.0, 4.0] {
                let limit = max_servable_zoom_permille(iso_a(n), dsf, budget_for(gib));
                assert_eq!(
                    limit, ZOOM_RANGE_MAX_PERMILLE,
                    "A{n} at {gib} GiB / {dsf}x was limited to {limit}; ordinary \
                     paper must reach full zoom on every device",
                );
            }
        }
    }
    // A0 on a small HiDPI device is the case the clamp exists for.
    let limit = max_servable_zoom_permille(iso_a(0), 4.0, budget_for(2.0));
    assert!(
        (1000..1750).contains(&limit),
        "A0 on 2 GiB at 4x reported a servable limit of {limit} permille; it was \
         measured as first exceeding at 1750, so the limit belongs just below that",
    );
}

/// Every measured combination is servable at *some* zoom, which is what makes a
/// clamp the right instrument rather than a load-time refusal.
///
/// If this ever fails, the failing row needs the refusal path instead — the
/// clamp would be offering a range in which every value is unservable.
#[test]
fn every_page_size_is_servable_at_minimum_zoom() {
    for n in 0..=4_usize {
        for gib in [2.0_f64, 4.0, 8.0, 16.0, 64.0] {
            for dsf in [1.0_f64, 2.0, 3.0, 4.0] {
                assert!(
                    is_servable_at_all(iso_a(n), dsf, budget_for(gib)),
                    "A{n} at {gib} GiB / {dsf}x cannot be served at minimum zoom, \
                     so no zoom clamp reaches it and a load-time refusal is the \
                     honest response for that row",
                );
            }
        }
    }
}

/// The search must never report a limit outside the range the control offers.
#[test]
fn the_reported_limit_stays_inside_the_zoom_clamp() {
    for n in 0..=4_usize {
        let limit = max_servable_zoom_permille(iso_a(n), 4.0, budget_for(2.0));
        assert!(
            (ZOOM_RANGE_MIN_PERMILLE..=ZOOM_RANGE_MAX_PERMILLE).contains(&limit),
            "A{n} reported {limit}, outside [{ZOOM_RANGE_MIN_PERMILLE}, {ZOOM_RANGE_MAX_PERMILLE}]",
        );
    }
}

/// The two bounds are ordered, and the gap between them is the whole reason a
/// "never reduce a zoom already in effect" rule can be stated at all.
///
/// Below the full-scale bound nothing is degraded. Between the two the page is
/// soft but the allocation is bounded — that band is where the rule applies, and
/// if it were empty the rule would have nowhere to live. Above the servable bound
/// the plan requests more than the OOM ceiling, and no policy about user
/// disruption can apply there.
#[test]
fn the_full_scale_bound_never_exceeds_the_servable_bound() {
    for n in 0..=4_usize {
        for gib in [2.0_f64, 4.0, 8.0, 16.0, 64.0] {
            for dsf in [1.0_f64, 2.0, 3.0, 4.0] {
                let page = iso_a(n);
                let budget = budget_for(gib);
                let sharp = max_full_scale_zoom_permille(page, dsf, budget);
                let servable = max_servable_zoom_permille(page, dsf, budget);
                assert!(
                    sharp <= servable,
                    "A{n} at {gib} GiB / {dsf}x reports full scale to {sharp} but \
                     servable only to {servable}; a zoom cannot be sharp and \
                     unservable at once",
                );
            }
        }
    }
}

/// The band is wide where it matters, so the non-retroactive rule protects a
/// real range rather than a rounding error.
///
/// A0 on 2 GiB at 4x was measured soft from 50% and over the ceiling from 175%:
/// most of the usable zoom range is degraded-but-safe, which is exactly the
/// territory a user should not be forcibly moved out of.
#[test]
fn the_soft_but_bounded_band_is_wide_on_the_extreme_device() {
    let budget = budget_for(2.0);
    let sharp = max_full_scale_zoom_permille(iso_a(0), 4.0, budget);
    let servable = max_servable_zoom_permille(iso_a(0), 4.0, budget);
    assert!(
        servable >= sharp + 500,
        "the soft-but-bounded band on A0 / 2 GiB / 4x is {sharp}..{servable} \
         permille, narrower than the 50 percentage points it was measured at — \
         a rule that lives in that band needs it to exist",
    );
}
