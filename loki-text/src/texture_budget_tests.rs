// SPDX-License-Identifier: Apache-2.0

//! Tests for the application-side budget join. Extracted per the file-ceiling
//! idiom.
//!
//! `current()` reads a Dioxus context, so what is pinned here is the mapping it
//! performs — which is where this module can be wrong independently of the
//! arithmetic it delegates to.

use appthere_canvas::residency::{BUDGET_FLOOR_BYTES, BudgetInputs, BudgetSource, TextureBudget};
use appthere_ui::{DeviceProfile, GpuClass};

/// The mapping `current()` applies, isolated from the context read.
fn inputs_for(profile: DeviceProfile, override_bytes: Option<u64>) -> BudgetInputs {
    BudgetInputs {
        available_ram_bytes: profile.available_ram_bytes,
        total_ram_bytes: profile.system_ram_bytes,
        gpu_paint_path: match profile.gpu_class {
            GpuClass::Unknown => None,
            other => Some(other.supports_gpu_paint()),
        },
        user_override_bytes: override_bytes,
    }
}

#[test]
fn an_unprobed_gpu_does_not_collapse_the_budget_to_the_floor() {
    // The failure this mapping exists to avoid: `Unknown` is "not looked yet",
    // and treating it as "no GPU" would floor the budget for the whole window
    // between app start and the first tile resuming — which is exactly the
    // window in which the first tiles are allocated.
    let profile = DeviceProfile {
        available_ram_bytes: Some(11 * 1024 * 1024 * 1024),
        gpu_class: GpuClass::Unknown,
        ..Default::default()
    };
    let budget = TextureBudget::derive(inputs_for(profile, None));
    assert_eq!(budget.source(), BudgetSource::AvailableRam);
    assert!(budget.bytes() > BUDGET_FLOOR_BYTES);
}

#[test]
fn a_software_rasteriser_reports_the_no_gpu_budget() {
    let profile = DeviceProfile {
        available_ram_bytes: Some(11 * 1024 * 1024 * 1024),
        gpu_class: GpuClass::Software,
        ..Default::default()
    };
    let budget = TextureBudget::derive(inputs_for(profile, None));
    assert_eq!(budget.source(), BudgetSource::NoGpuPaintPath);
}

#[test]
fn an_entirely_unprobed_profile_gets_the_baseline() {
    // R24's state. It must be a working budget, not a throttled one.
    let budget = TextureBudget::derive(inputs_for(DeviceProfile::default(), None));
    assert_eq!(budget.source(), BudgetSource::Baseline);
}

#[test]
fn the_override_reaches_the_derivation() {
    let profile = DeviceProfile {
        available_ram_bytes: Some(1024 * 1024 * 1024),
        gpu_class: GpuClass::Integrated,
        ..Default::default()
    };
    let budget = TextureBudget::derive(inputs_for(profile, Some(512 * 1024 * 1024)));
    assert_eq!(budget.source(), BudgetSource::UserOverride);
    assert_eq!(budget.bytes(), 512 * 1024 * 1024);
}

#[test]
fn the_same_ram_gives_the_same_budget_on_every_device_class() {
    // L08-011 as this application sees it: two profiles that differ only in
    // things that are not memory must produce the same budget.
    let a = DeviceProfile {
        available_ram_bytes: Some(8 * 1024 * 1024 * 1024),
        gpu_class: GpuClass::Discrete,
        hardware_keyboard: true,
        ..Default::default()
    };
    let b = DeviceProfile {
        available_ram_bytes: Some(8 * 1024 * 1024 * 1024),
        gpu_class: GpuClass::Integrated,
        hardware_keyboard: false,
        ..Default::default()
    };
    assert_eq!(
        TextureBudget::derive(inputs_for(a, None)).bytes(),
        TextureBudget::derive(inputs_for(b, None)).bytes(),
    );
}
