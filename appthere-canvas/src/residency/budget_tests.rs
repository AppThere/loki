// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the budget derivation. Extracted per the file-ceiling idiom.

use super::{
    AVAILABLE_RAM_DIVISOR, BUDGET_BASELINE_BYTES, BUDGET_CEILING_BYTES, BUDGET_FLOOR_BYTES,
    BudgetInputs, BudgetSource, SURVIVAL_AVAILABLE_RAM_DIVISOR, SURVIVAL_CAP_BYTES,
    SURVIVAL_TOTAL_RAM_DIVISOR, TOTAL_RAM_DIVISOR, TextureBudget,
};

const GIB: u64 = 1024 * 1024 * 1024;

#[test]
fn an_unknown_device_gets_the_baseline_not_the_floor() {
    // The direction that matters: an input nobody filled in must not silently
    // throttle the renderer. This is also the state R24 describes — every probe
    // returning Unknown — so it is the state the tree was in before T2.0.
    let b = TextureBudget::derive(BudgetInputs::default());
    assert_eq!(b.bytes(), BUDGET_BASELINE_BYTES);
    assert_eq!(b.source(), BudgetSource::Baseline);
}

#[test]
fn the_two_ram_paths_agree_at_the_design_floor() {
    // Spec 06's design floor is an 8 GB machine with a browser and an OS also
    // resident, so ~4 GiB available. Both derivations must land on the same
    // number there, or the fallback is a different policy rather than a
    // degradation of the same one.
    let from_available = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(4 * GIB),
        ..Default::default()
    });
    let from_total = TextureBudget::derive(BudgetInputs {
        total_ram_bytes: Some(8 * GIB),
        ..Default::default()
    });
    assert_eq!(from_available.bytes(), from_total.bytes());
    assert_eq!(from_available.bytes(), BUDGET_BASELINE_BYTES);
    assert_eq!(from_available.source(), BudgetSource::AvailableRam);
    assert_eq!(from_total.source(), BudgetSource::TotalRam);
}

#[test]
fn available_ram_wins_over_total() {
    // Total says what the machine has; available says what is ours. A machine
    // with 32 GiB installed but 2 GiB free is a 2 GiB machine for our purposes.
    let b = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(2 * GIB),
        total_ram_bytes: Some(32 * GIB),
        ..Default::default()
    });
    assert_eq!(b.bytes(), 2 * GIB / 64);
    assert_eq!(b.source(), BudgetSource::AvailableRam);
}

#[test]
fn a_large_machine_is_capped_and_a_small_one_is_floored() {
    let big = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(64 * GIB),
        ..Default::default()
    });
    assert_eq!(big.bytes(), BUDGET_CEILING_BYTES);

    let small = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(512 * 1024 * 1024),
        ..Default::default()
    });
    assert_eq!(small.bytes(), BUDGET_FLOOR_BYTES);
}

#[test]
fn the_same_ram_gives_the_same_budget_whatever_the_device_is() {
    // L08-011 in one assertion: the derivation reads measured quantities, so an
    // Android laptop with 16 GiB and a desktop with 16 GiB cannot diverge. If a
    // `cfg!(target_os)` ever creeps into the derivation, this test cannot catch
    // it — but the inputs type has nowhere to put one, which is the real guard.
    let inputs = BudgetInputs {
        available_ram_bytes: Some(11 * GIB),
        total_ram_bytes: Some(16 * GIB),
        gpu_paint_path: Some(true),
        user_override_bytes: None,
        diagnostic_ceiling_bytes: None,
    };
    assert_eq!(
        TextureBudget::derive(inputs).bytes(),
        11 * GIB / 64,
        "a 16 GiB machine with 11 GiB free gets 176 MiB, whatever it is",
    );
}

#[test]
fn a_user_override_beats_everything_and_is_not_capped_by_the_ceiling() {
    // T2.1: "always user-overridable". The ceiling bounds an automatic
    // derivation on a big machine; it does not overrule a person.
    let b = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(512 * 1024 * 1024),
        user_override_bytes: Some(1024 * 1024 * 1024),
        ..Default::default()
    });
    assert_eq!(b.bytes(), 1024 * 1024 * 1024);
    assert_eq!(b.source(), BudgetSource::UserOverride);
}

#[test]
fn a_user_override_below_the_floor_is_raised_to_it() {
    // Below the floor a single visible page cannot be held even at the
    // reduced-scale limit, so the setting would be a blank screen rather than a
    // memory saving.
    let b = TextureBudget::derive(BudgetInputs {
        user_override_bytes: Some(1),
        ..Default::default()
    });
    assert_eq!(b.bytes(), BUDGET_FLOOR_BYTES);
}

#[test]
fn no_gpu_paint_path_reports_the_floor_with_its_reason() {
    let b = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(64 * GIB),
        gpu_paint_path: Some(false),
        ..Default::default()
    });
    assert_eq!(b.bytes(), BUDGET_FLOOR_BYTES);
    assert_eq!(b.source(), BudgetSource::NoGpuPaintPath);
}

#[test]
fn an_unprobed_gpu_does_not_throttle() {
    // `None` is "not looked yet", not "no GPU" (L9-009). Treating it as the
    // latter would put every session on the floor until the first tile paints.
    let b = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(11 * GIB),
        gpu_paint_path: None,
        ..Default::default()
    });
    assert_eq!(b.bytes(), 11 * GIB / 64);
}

#[test]
fn the_survival_ceiling_is_capped_absolutely_and_not_only_proportionally() {
    // L08-027's second half: a purely proportional ceiling has no opinion about
    // absurdity. A 64 GiB workstation reporting ~50 GiB available would derive
    // 6.4 GiB of texture ceiling for a word processor.
    let big = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(50 * 1024 * 1024 * 1024),
        ..BudgetInputs::default()
    });
    assert_eq!(
        big.hard_ceiling_bytes(),
        SURVIVAL_CAP_BYTES,
        "a large machine is capped, not scaled without limit",
    );

    // The cap binds only where the proportional figure exceeds it — a smaller
    // machine still gets its own, smaller ceiling.
    let floor = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(4 * 1024 * 1024 * 1024),
        ..BudgetInputs::default()
    });
    assert_eq!(floor.hard_ceiling_bytes(), 512 * 1024 * 1024);
    assert!(floor.hard_ceiling_bytes() < SURVIVAL_CAP_BYTES);

    // And the ceiling >= target invariant survives the cap: a person may set a
    // target above it, and the cap must not then invert the two.
    let over = TextureBudget::with_baseline_ceiling(2 * 1024 * 1024 * 1024);
    assert!(
        over.hard_ceiling_bytes() >= over.bytes(),
        "an explicit target above the cap must not end up above its own ceiling",
    );
}

/// **The two platform paths must not diverge silently.**
///
/// Linux reports *available* RAM and takes the `AVAILABLE_RAM_DIVISOR` path;
/// macOS reports only *total* and takes `TOTAL_RAM_DIVISOR` (see
/// `appthere_ui::device_probe` for why a guessed available is worse than none).
/// The same physical machine should therefore land on comparable budgets under
/// both — and nothing enforced that, so tuning either constant alone would move
/// one platform and not the other, silently.
///
/// **The relation is `total = 2 x available`**, and it encodes a stated
/// assumption: a loaded machine reports roughly half its RAM as available. That
/// is Spec 06's design-floor calibration point — an 8 GiB machine with a browser
/// and an OS resident reports ~4 GiB, and `4 GiB / 64` and `8 GiB / 128` are the
/// same number by construction.
///
/// Pinned for the survival ceiling too, which carries the same pair (8 / 16).
#[test]
fn the_total_and_available_divisors_stay_calibrated_against_each_other() {
    assert_eq!(
        TOTAL_RAM_DIVISOR,
        2 * AVAILABLE_RAM_DIVISOR,
        "the total path assumes available is half of total; changing one \
         divisor without the other makes Linux and macOS disagree about the \
         same machine",
    );
    assert_eq!(
        SURVIVAL_TOTAL_RAM_DIVISOR,
        2 * SURVIVAL_AVAILABLE_RAM_DIVISOR,
        "same relation, same reason, for the survival ceiling",
    );
}

/// The relation asserted as the outcome it exists for: **one machine, two
/// platforms, the same budget** — and a bounded disagreement when the machine is
/// more or less loaded than the assumption.
///
/// The band is not a fudge factor; it is the assumption's own sensitivity. A
/// budget from available is `T x r / 64` and from total is `T / 128`, so the
/// ratio is exactly `2r`: an available fraction anywhere in 0.35-0.65 keeps the
/// two platforms within 30% of each other, which is the honest statement of how
/// much the platform split can cost.
#[test]
fn one_machine_lands_on_the_same_budget_whichever_figure_its_platform_reports() {
    for gib in [4_u64, 8, 16, 32] {
        let total = gib * GIB;
        let from_total = TextureBudget::derive(BudgetInputs {
            total_ram_bytes: Some(total),
            ..BudgetInputs::default()
        });
        // The assumption, exactly: half the machine is available.
        let from_available = TextureBudget::derive(BudgetInputs {
            available_ram_bytes: Some(total / 2),
            ..BudgetInputs::default()
        });
        assert_eq!(
            from_total.bytes(),
            from_available.bytes(),
            "{gib} GiB: total path gave {} and available path gave {} — the two \
             platforms disagree about one machine",
            from_total.bytes(),
            from_available.bytes(),
        );
        assert_eq!(
            from_total.hard_ceiling_bytes(),
            from_available.hard_ceiling_bytes(),
            "{gib} GiB: the survival ceilings disagree",
        );

        // And the sensitivity, at the edges of a plausible load.
        for pct in [35_u64, 65] {
            let loaded = TextureBudget::derive(BudgetInputs {
                available_ram_bytes: Some(total * pct / 100),
                ..BudgetInputs::default()
            });
            let ratio = loaded.bytes() as f64 / from_total.bytes() as f64;
            assert!(
                (0.69..=1.31).contains(&ratio),
                "{gib} GiB at {pct}% available: the available path is {ratio:.2}x \
                 the total path, outside the band the calibration claims",
            );
        }
    }
}

/// **The r18 defect, reproduced inside `derive`.** The override sets the target;
/// the survival ceiling is a fact about the machine and stays the machine's.
///
/// Before the fix this called `with_baseline_ceiling`, which supplies the 4
/// GiB-available machine's ceiling whatever the device — so on a small machine a
/// *lower* override produced a *higher* ceiling.
#[test]
fn a_user_override_does_not_move_the_devices_survival_ceiling() {
    let available = 2 * GIB;
    let derived = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(available),
        ..Default::default()
    });
    let overridden = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(available),
        // Deliberately *less* memory than the derivation chose.
        user_override_bytes: Some(48 * 1024 * 1024),
        ..Default::default()
    });
    assert_eq!(
        overridden.hard_ceiling_bytes(),
        derived.hard_ceiling_bytes(),
        "asking for a smaller budget must not raise the line past which visible \
         pages are degraded — it moved from {} to {} MiB before the fix",
        derived.hard_ceiling_bytes() / (1024 * 1024),
        overridden.hard_ceiling_bytes() / (1024 * 1024),
    );
    assert_eq!(
        overridden.bytes(),
        48 * 1024 * 1024,
        "the target is the user's"
    );
}

/// The polarity that keeps the assertion above from being satisfied by pinning
/// the ceiling to a constant: a bigger machine still gets a bigger ceiling under
/// an override.
#[test]
fn the_overridden_ceiling_still_tracks_the_device() {
    let small = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(2 * GIB),
        user_override_bytes: Some(48 * 1024 * 1024),
        ..Default::default()
    });
    let large = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(16 * GIB),
        user_override_bytes: Some(48 * 1024 * 1024),
        ..Default::default()
    });
    assert!(
        large.hard_ceiling_bytes() > small.hard_ceiling_bytes(),
        "same override, 8x the RAM, same ceiling — the device stopped mattering",
    );
}

/// With nothing known about the machine there is no device ceiling to keep, so
/// the baseline is correct rather than a fallback that hides a missing input.
#[test]
fn an_override_with_no_memory_reading_keeps_the_baseline_ceiling() {
    let b = TextureBudget::derive(BudgetInputs {
        user_override_bytes: Some(48 * 1024 * 1024),
        ..Default::default()
    });
    assert_eq!(
        b.hard_ceiling_bytes(),
        TextureBudget::baseline().hard_ceiling_bytes(),
    );
}

/// **The diagnostic ceiling moves the ceiling and only the ceiling.** It exists
/// because r67 correctly stopped the budget override moving it, which removed
/// R5b's only route into the survival regime (L08-052).
#[test]
fn the_diagnostic_ceiling_lowers_the_ceiling_without_touching_the_target() {
    let plain = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(11 * GIB),
        ..Default::default()
    });
    let forced = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(11 * GIB),
        diagnostic_ceiling_bytes: Some(300 * 1024 * 1024),
        ..Default::default()
    });
    assert_eq!(
        forced.bytes(),
        plain.bytes(),
        "the target is not this lever's business — one lever, one quantity",
    );
    assert_eq!(forced.hard_ceiling_bytes(), 300 * 1024 * 1024);
    assert!(
        plain.hard_ceiling_bytes() > forced.hard_ceiling_bytes(),
        "precondition: the fixture must actually lower something — an 11 GiB \
         machine derives {} MiB, which must exceed the forced 300",
        plain.hard_ceiling_bytes() / (1024 * 1024),
    );
    assert_eq!(
        forced.source(),
        plain.source(),
        "and it must not rewrite how the budget was arrived at",
    );
}

/// It cannot be used to put the ceiling *below* the target, because a budget
/// whose ceiling sits under its own target is not a state the planner has a
/// meaning for. The invariant wins and the app layer says so.
#[test]
fn a_diagnostic_ceiling_under_the_target_is_raised_to_it() {
    let b = TextureBudget::derive(BudgetInputs {
        available_ram_bytes: Some(11 * GIB),
        diagnostic_ceiling_bytes: Some(1024 * 1024),
        ..Default::default()
    });
    assert_eq!(b.hard_ceiling_bytes(), b.bytes());
}

/// The polarity: absent, it changes nothing at all. Without this the lever could
/// be a constant and both assertions above would still pass.
#[test]
fn no_diagnostic_ceiling_leaves_every_arm_alone() {
    for inputs in [
        BudgetInputs {
            available_ram_bytes: Some(11 * GIB),
            ..Default::default()
        },
        BudgetInputs {
            total_ram_bytes: Some(8 * GIB),
            ..Default::default()
        },
        BudgetInputs {
            user_override_bytes: Some(48 * 1024 * 1024),
            available_ram_bytes: Some(2 * GIB),
            ..Default::default()
        },
        BudgetInputs::default(),
    ] {
        let with_none = TextureBudget::derive(BudgetInputs {
            diagnostic_ceiling_bytes: None,
            ..inputs
        });
        assert_eq!(TextureBudget::derive(inputs), with_none);
    }
}
