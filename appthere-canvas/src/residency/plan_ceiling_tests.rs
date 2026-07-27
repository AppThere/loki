// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Where step 5's floor binds and the ceiling is still exceeded (Spec 08 r30).
//!
//! # The branch that knowingly over-allocates
//!
//! `plan_depth_tests` establishes that softness is bounded at
//! [`MIN_RASTER_SCALE`]. That bound has a cost the depth measurement does not
//! show: when the scale search reaches the floor and the visible set *still*
//! exceeds the survival ceiling, the loop breaks and mounts anyway. The plan then
//! requests more than the threshold calibrated against the OOM killer.
//!
//! # It is reachable today, and not only on small machines
//!
//! Needed scale goes as `1/sqrt(page area)` and the ISO series doubles area at
//! each step, so paper size walks straight into the floor. Measured, over the
//! whole clamped zoom range and 40 offsets:
//!
//! | page | 2 GiB / 4x | 8-16 GiB / 4x |
//! | --- | --- | --- |
//! | A4 | 0.376 | 0.752 |
//! | A3 | 0.266 | 0.542 |
//! | A2 | **floor**, +179 MiB over | 0.376 |
//! | A1 | **floor**, +615 MiB over | 0.271 |
//! | A0 | **floor**, +1488 MiB over | **floor**, +720 MiB over |
//!
//! **A0 reaches it on a 16 GiB machine**, which is stronger than "a constrained
//! device": the ceiling saturates at `SURVIVAL_CAP_BYTES` while page area does
//! not saturate at all.
//!
//! And it is reachable *now*, not pending a page-size catalogue — only the
//! [`PageBox`] constructors are limited to A4 and Letter, while imported page
//! geometry is arbitrary. A hand-written A0 `w:pgSz` is a few bytes of XML.
//!
//! # What is decided here, and what is not
//!
//! Decided: it will not be silent. `ResidencyPlan::ceiling_exceeded` says so.
//!
//! Not decided: what the application does about it. There is a real option the
//! policy does not currently have — **drop to the single centre page**, which
//! these measurements show would fit under the ceiling at the floor in four of
//! the six exceeding rows. That would change the "visible tiles are never
//! dropped" acceptance criterion, which was calibrated against a *reading*
//! scenario rather than a 400%-zoom one where the second visible page is a
//! sliver at the edge. Changing an acceptance criterion is a spec decision, so it
//! is recorded rather than taken.

use super::super::budget::{BudgetInputs, TextureBudget};
use super::super::geometry::{MAX_ZOOM, MIN_ZOOM, PageBox, ViewportSpec};
use super::{MIN_RASTER_SCALE, plan_residency};

/// ISO A series in points, A4 through A0.
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

/// Worst overshoot above the ceiling across the clamped range, and whether the
/// single smallest visible tile would have fitted at the floor.
fn worst_overshoot(page: PageBox, available_gib: f64, dsf: f64) -> (u64, bool) {
    let doc = vec![page; 500];
    let budget = budget_for(available_gib);
    let ceiling = budget.hard_ceiling_bytes();
    let mut over = 0_u64;
    let mut one_page_would_fit = true;
    let mut zoom = MIN_ZOOM;
    while zoom <= MAX_ZOOM + f64::EPSILON {
        for offset in 0..40 {
            let vp = ViewportSpec::new(20_000.0 + f64::from(offset) * 500.0, 900.0, zoom, dsf);
            let plan = plan_residency(&doc, &vp, budget);
            if !plan.ceiling_exceeded {
                continue;
            }
            over = over.max(plan.total_bytes.saturating_sub(ceiling));
            if plan
                .tiles
                .iter()
                .map(|t| t.bytes)
                .min()
                .is_some_and(|b| b > ceiling)
            {
                one_page_would_fit = false;
            }
        }
        zoom += 0.25;
    }
    (over, one_page_would_fit)
}

/// The flag is not decorative: ordinary imported paper reaches it.
#[test]
fn large_paper_exceeds_the_ceiling_at_the_floor() {
    let (over, _) = worst_overshoot(iso_a(1), 2.0, 4.0);
    assert!(
        over > 0,
        "A1 on 2 GiB at 4x did not exceed the ceiling; the floor no longer binds \
         there and the reachability table needs re-measuring",
    );
}

/// The claim that matters most, because it is the one that sounds wrong: a large
/// machine is not safe from this. The ceiling saturates at the absolute cap;
/// page area does not saturate.
#[test]
fn a_sixteen_gib_machine_still_exceeds_the_ceiling_on_a0() {
    let (over, _) = worst_overshoot(iso_a(0), 16.0, 4.0);
    assert!(
        over > 512 * 1024 * 1024,
        "A0 on 16 GiB at 4x overshot by {over} B; it was measured at ~720 MiB. \
         If this fell below half a gibibyte the cap or the demand curve moved",
    );
}

/// Ordinary paper must *not* reach it, or the flag says nothing by being always
/// set. A4 and A3 are the sizes real documents use.
#[test]
fn ordinary_paper_never_exceeds_the_ceiling() {
    for n in [4_usize, 3] {
        for gib in [2.0_f64, 4.0, 8.0, 16.0, 64.0] {
            for dsf in [1.0_f64, 2.0, 3.0, 4.0] {
                let (over, _) = worst_overshoot(iso_a(n), gib, dsf);
                assert_eq!(
                    over, 0,
                    "A{n} at {gib} GiB / {dsf}x exceeded the ceiling by {over} B — \
                     this flag is meant to mark the case the policy cannot serve, \
                     not ordinary operation",
                );
            }
        }
    }
}

/// The flag never fires without the regime it belongs to, so a reader of a
/// diagnostic can trust the pair.
#[test]
fn exceeding_the_ceiling_implies_the_survival_regime() {
    let doc = vec![iso_a(0); 500];
    let budget = budget_for(2.0);
    let mut seen = false;
    let mut zoom = MIN_ZOOM;
    while zoom <= MAX_ZOOM + f64::EPSILON {
        for offset in 0..40 {
            let vp = ViewportSpec::new(20_000.0 + f64::from(offset) * 500.0, 900.0, zoom, 4.0);
            let plan = plan_residency(&doc, &vp, budget);
            if plan.ceiling_exceeded {
                seen = true;
                assert!(plan.survival_reduced, "exceeded without survival_reduced");
                assert!(
                    plan.over_target,
                    "exceeding the ceiling implies over target"
                );
                for tile in plan.tiles.iter().filter(|t| t.visible) {
                    assert!(
                        (tile.raster_scale - MIN_RASTER_SCALE).abs() < 1e-6,
                        "exceeded at scale {} rather than the floor — the search \
                         gave up early",
                        tile.raster_scale,
                    );
                }
            }
        }
        zoom += 0.25;
    }
    assert!(
        seen,
        "the fixture never reached the branch, so this asserts nothing"
    );
}

/// The recorded, undecided option: in most exceeding cases a single page *would*
/// fit at the floor, so "mount both over the ceiling" is not the only move
/// available. Pinned so the option does not quietly stop existing.
#[test]
fn a_single_page_would_often_have_fitted() {
    let (over, one_fits) = worst_overshoot(iso_a(0), 16.0, 4.0);
    assert!(over > 0);
    assert!(
        one_fits,
        "on 16 GiB at 4x even one A0 page exceeds the ceiling at the floor, which \
         removes the drop-to-centre-page option there — the spec's recorded \
         alternative needs revisiting",
    );
}
