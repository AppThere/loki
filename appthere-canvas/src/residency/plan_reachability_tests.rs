// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Is the survival regime — `plan_residency` step 5, the only place this planner
//! degrades what the reader is looking at — reachable at all?
//!
//! # Why this needed asking
//!
//! Phase 2's screen session never reached it: `survival_reduced` read `false` on
//! every line, on a 2x display, even with the budget forced to 24 MiB. That is the
//! predicted outcome, but "predicted" and "unreachable" are the same observation
//! from outside. Code that can never run is either a ceiling set too generously or
//! a path that should not ship, and Phase 2 should close knowing which.
//!
//! Zoom is clamped to [`MAX_ZOOM`], so this is a bounded question rather than an
//! open one, and answerable without a device.
//!
//! # The answer, and it is not the intuitive one
//!
//! Reachability runs **opposite** to memory size. The survival ceiling is
//! `min(available / 8, 1 GiB)` floored at the target, so it saturates at the
//! `SURVIVAL_CAP_BYTES` cap from 8 GiB available upward — while peak texture
//! demand keeps rising with page size and display scale. Small machines have a
//! proportionally small ceiling and reach it easily; large ones sit behind a fixed
//! 1 GiB wall that US Letter cannot quite climb.
//!
//! Measured peak *visible* bytes and the lowest zoom at which step 5 fires, over
//! 40 scroll offsets per zoom in 0.25 steps up to [`MAX_ZOOM`]:
//!
//! | available | display | ceiling | US Letter | A3 |
//! | ---: | ---: | ---: | --- | --- |
//! | 2 GiB | 2x | 256 MiB | fires at 325% | fires at 125% (4x) |
//! | 2 GiB | 3x | 256 MiB | fires at 225% | fires at 150% |
//! | 4 GiB | 3x | 512 MiB | fires at 300% | fires at 225% |
//! | 8 GiB+ | 3x | 1024 MiB | **never** — peaks at 946.7 MiB | fires at 300% |
//! | 8 GiB+ | 4x | 1024 MiB | fires | fires at 225% |
//!
//! So step 5 is live code on every memory size, and the case that does *not*
//! reach it — US Letter, 8 GiB or more, 3x — misses by **77 MiB in 1024, about
//! 7.5%**. That margin is the number worth watching: it is not headroom, it is a
//! near-miss that a larger paper size or one more display-scale step converts
//! into a hit. The `A3` column is that same machine with a bigger page.

use super::super::budget::{BudgetInputs, TextureBudget};
use super::super::geometry::{MAX_ZOOM, PageBox, ViewportSpec};
use super::plan_residency;

/// A3 in points — the largest paper size an ordinary word processor offers, and
/// the one that decides reachability on a large machine.
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

/// Sweeps the reachable operating space for one page size and device, returning
/// the lowest zoom at which step 5 fires and the peak visible byte figure seen.
///
/// Scroll offsets are swept as well as zoom, because the visible set is one page
/// at most offsets and two where a page boundary crosses the viewport — and it is
/// the two-page offsets that peak. A single offset would under-report by half and
/// answer the reachability question wrongly in the safe direction.
fn sweep(page: PageBox, available_gib: f64, dsf: f64) -> (Option<f64>, u64) {
    let doc = vec![page; 500];
    let budget = budget_for(available_gib);
    let mut fires_at = None;
    let mut peak = 0_u64;
    let mut zoom = super::super::geometry::MIN_ZOOM;
    while zoom <= MAX_ZOOM + f64::EPSILON {
        for offset in 0..40 {
            let vp = ViewportSpec::new(20_000.0 + f64::from(offset) * 500.0, 900.0, zoom, dsf);
            let plan = plan_residency(&doc, &vp, budget);
            peak = peak.max(
                plan.tiles
                    .iter()
                    .filter(|t| t.visible)
                    .map(|t| t.bytes)
                    .sum(),
            );
            if plan.survival_reduced && fires_at.is_none() {
                fires_at = Some(zoom);
            }
        }
        zoom += 0.25;
    }
    (fires_at, peak)
}

/// Step 5 is not dead code: it fires inside the zoom the app itself permits, on
/// every memory size, for a page size the app offers.
///
/// Asserted per memory size rather than "somewhere in the grid", because a single
/// reachable cell would let the path be dead on every machine but one and still
/// pass.
#[test]
fn the_survival_regime_is_reachable_within_the_zoom_clamp() {
    for available_gib in [2.0_f64, 4.0, 8.0, 16.0, 32.0, 64.0] {
        let (fires_at, _) = sweep(a3(), available_gib, 3.0);
        assert!(
            fires_at.is_some(),
            "the survival regime is unreachable on a {available_gib} GiB machine at \
             any zoom up to {MAX_ZOOM} on a 3x display, even at A3 — step 5 would \
             be dead code there, which is a ceiling set too generously rather than \
             a policy",
        );
    }
}

/// A small machine reaches it on ordinary paper at ordinary zoom, which is the
/// case that makes step 5 worth having rather than merely reachable.
#[test]
fn a_low_memory_device_reaches_it_on_us_letter() {
    let (fires_at, _) = sweep(PageBox::us_letter(), 2.0, 2.0);
    assert_eq!(
        fires_at,
        Some(3.25),
        "a 2 GiB / 2x device should enter the survival regime at 325% zoom on US \
         Letter; if this moved, the ceiling or the demand curve changed",
    );
}

/// The near-miss, pinned as a number rather than left as an impression.
///
/// US Letter on 8 GiB+ at 3x peaks just under the 1 GiB cap and never reaches
/// step 5. This asserts *both* halves — that it does not fire, and that the
/// margin is thin — so a change that widens the gap to comfortable is as visible
/// as one that closes it. A comment saying "about 7.5%" would go stale silently;
/// this cannot.
#[test]
fn the_large_machine_near_miss_stays_a_near_miss() {
    let (fires_at, peak) = sweep(PageBox::us_letter(), 16.0, 3.0);
    assert_eq!(
        fires_at, None,
        "expected US Letter at 3x not to reach step 5"
    );
    let ceiling = budget_for(16.0).hard_ceiling_bytes();
    let margin = 1.0 - (peak as f64 / ceiling as f64);
    assert!(
        (0.05..0.10).contains(&margin),
        "peak visible demand is {peak} B against a {ceiling} B ceiling — a margin \
         of {:.1}%, outside the 5-10% band this near-miss was measured at. Below \
         5% the regime is effectively reachable here and the case above should \
         cover it; above 10% something reduced peak demand or raised the cap, and \
         the reachability table in these docs needs re-measuring.",
        margin * 100.0,
    );
}

/// Raising the zoom clamp changes the answer, so the clamp has to be the thing
/// the sweep reads rather than a literal that happens to agree with it.
///
/// The coupling this protects runs the other way too — `loki-renderer`'s
/// `set_zoom` must clamp at the same constant, which
/// `doc_page_source_scale::tests::the_zoom_clamp_is_the_shared_residency_bound`
/// asserts from that side. Split across the two crates because neither can see
/// the other's half: this one owns the demand curve, that one owns the clamp.
#[test]
fn the_sweep_covers_the_whole_clamped_range() {
    let (_, peak_at_max) = sweep(PageBox::us_letter(), 16.0, 3.0);
    let doc = vec![PageBox::us_letter(); 500];
    let at_max = ViewportSpec::new(20_000.0, 900.0, MAX_ZOOM, 3.0);
    let plan = plan_residency(&doc, &at_max, budget_for(16.0));
    let visible: u64 = plan
        .tiles
        .iter()
        .filter(|t| t.visible)
        .map(|t| t.bytes)
        .sum();
    assert!(
        peak_at_max >= visible,
        "the sweep peaked at {peak_at_max} B but the clamp's own top end demands \
         {visible} B — the sweep is not reaching {MAX_ZOOM}, so every reachability \
         answer above is measured over less than the app permits",
    );
}
