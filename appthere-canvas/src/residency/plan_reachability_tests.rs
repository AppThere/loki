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
//! Zoom is clamped to [`ZOOM_RANGE_MAX`], so this is a bounded question rather than an
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
//! 40 scroll offsets per zoom in 0.25 steps up to [`ZOOM_RANGE_MAX`]:
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
use super::super::geometry::{PageBox, ViewportSpec, ZOOM_RANGE_MAX};
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
    let mut zoom = super::super::geometry::ZOOM_RANGE_MIN;
    while zoom <= ZOOM_RANGE_MAX + f64::EPSILON {
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
             any zoom up to {ZOOM_RANGE_MAX} on a 3x display, even at A3 — step 5 would \
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
    let at_max = ViewportSpec::new(20_000.0, 900.0, ZOOM_RANGE_MAX, 3.0);
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
         {visible} B — the sweep is not reaching {ZOOM_RANGE_MAX}, so every reachability \
         answer above is measured over less than the app permits",
    );
}

/// A budget derived the way a **total-only** platform derives it — macOS today,
/// and Windows if it ships without an available-memory probe.
fn budget_from_total(total_gib: f64) -> TextureBudget {
    TextureBudget::derive(BudgetInputs {
        available_ram_bytes: None,
        total_ram_bytes: Some((total_gib * 1024.0 * 1024.0 * 1024.0) as u64),
        gpu_paint_path: Some(true),
        user_override_bytes: None,
    })
}

/// The same sweep, against a budget supplied rather than derived from available.
fn sweep_with(page: PageBox, budget: TextureBudget, dsf: f64) -> (Option<f64>, u64) {
    let doc = vec![page; 500];
    let mut fires_at = None;
    let mut peak = 0_u64;
    let mut zoom = super::super::geometry::ZOOM_RANGE_MIN;
    while zoom <= ZOOM_RANGE_MAX + f64::EPSILON {
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

/// **Does a total-only probe cost any behaviour? Measured: yes, in 26 of 72
/// cells** — the Windows decision taken with a counter rather than a preference
/// (Spec 08 r55).
///
/// Windows can report a meaningful *available* figure, but only through
/// `GlobalMemoryStatusEx`, which needs either a dependency or a documented
/// `unsafe` exception. The alternative was to do what macOS does and report total
/// only, and the argument for it was the calibration band: the two paths agree
/// within 30% across a 35–65% available fraction, so perhaps the fidelity is
/// below the resolution that matters.
///
/// **It is not.** 30% of budget turns out to be 30% of behaviour often enough to
/// decide the question: over machine size × display scale × page size × load,
/// more than a third of the grid moves the zoom at which the survival regime
/// fires — the one place this planner degrades what the reader is looking at.
///
/// # The error is asymmetric, and the dangerous side is a loaded machine
///
/// At **35%** available — a machine under real load, which is the case the whole
/// available-RAM design exists for — total-only fires **later or not at all**.
/// The sharpest cell is 8 GiB, 2×, US Letter: available-based fires at 375%,
/// total-only **never**. Never firing is not "less cautious"; it means mounting
/// at full scale against a ceiling derived from RAM the machine does not have,
/// which is the OOM branch §3.6e exists to keep out of.
///
/// At **65%** available the error runs the other way and is merely wasteful:
/// total-only fires earlier than needed, softening text on a machine with room
/// to spare.
///
/// So the measured answer is that Windows should pay for the available figure —
/// dependency or exception — rather than inherit macOS's total-only shape. And
/// `windows-sys` does not avoid the exception: it supplies declarations, not safe
/// wrappers, so the call is still `unsafe`. That makes the real choice
/// `sysinfo` (safe API, heaviest dependency) against `windows-sys` (exception
/// required, correct signature from Microsoft's metadata, near-zero transitive
/// deps) — and hand-rolling only wins under an absolute zero-dependency rule,
/// since it costs the same exception for a signature you must get right yourself.
///
/// # This test asserts the finding, not a guard
///
/// It fails if the two paths ever become equivalent, which would mean either the
/// divisors were re-tuned into agreement or the regime boundary moved. Both are
/// reasons to re-open the Windows decision rather than to quietly keep it.
#[test]
fn a_total_only_probe_changes_the_survival_regime_across_the_load_band() {
    let mut compared = 0_u32;
    let mut differed = 0_u32;
    // The safety-relevant cell, named so a later edit cannot lose it in an
    // aggregate: a loaded 8 GiB machine at 2x on US Letter.
    let mut never_fires_when_it_should = false;
    for total_gib in [4.0_f64, 8.0, 16.0, 32.0] {
        for dsf in [2.0_f64, 3.0, 4.0] {
            for page in [PageBox::us_letter(), a3()] {
                let (total_fires, _) = sweep_with(page, budget_from_total(total_gib), dsf);
                for pct in [35.0_f64, 50.0, 65.0] {
                    let (avail_fires, _) = sweep(page, total_gib * pct / 100.0, dsf);
                    compared += 1;
                    if total_fires != avail_fires {
                        differed += 1;
                    }
                    if total_fires.is_none() && avail_fires.is_some() {
                        never_fires_when_it_should = true;
                    }
                }
            }
        }
    }
    assert_eq!(compared, 72, "the grid changed shape: {compared} cells");
    assert!(
        differed >= 20,
        "only {differed} of {compared} cells differ — if the two paths have \
         converged, the Windows decision (r55: pay for the available figure) \
         rests on a measurement that no longer holds and must be re-taken",
    );
    assert!(
        never_fires_when_it_should,
        "no cell where total-only never degrades while available-based does — \
         that case is the reason the fidelity matters rather than merely differs, \
         and losing it changes the argument",
    );
    // The agreement point, as a control: at exactly half, the paths coincide by
    // construction, so a difference there would mean the calibration is broken
    // rather than that the platforms differ.
    for total_gib in [4.0_f64, 8.0, 16.0, 32.0] {
        let (t, _) = sweep_with(PageBox::us_letter(), budget_from_total(total_gib), 3.0);
        let (a, _) = sweep(PageBox::us_letter(), total_gib / 2.0, 3.0);
        assert_eq!(
            t, a,
            "{total_gib} GiB at exactly 50% available must agree — that is the \
             calibration point, and disagreement there is a broken relation \
             rather than a platform difference",
        );
    }
}
