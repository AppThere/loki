// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the budget derivation. Extracted per the file-ceiling idiom.

use super::{
    BUDGET_BASELINE_BYTES, BUDGET_CEILING_BYTES, BUDGET_FLOOR_BYTES, BudgetInputs, BudgetSource,
    TextureBudget,
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
