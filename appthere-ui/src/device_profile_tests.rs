// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::DeviceProfile`] and the pointer-precision latch.

use super::{DeviceProfile, GpuClass, PointerPrecision, WindowMode};

#[test]
fn everything_starts_unknown() {
    let p = DeviceProfile::default();
    assert_eq!(p.pointer, PointerPrecision::Unknown);
    assert_eq!(p.gpu_class, GpuClass::Unknown);
    assert_eq!(p.window_mode, WindowMode::Unknown);
    assert_eq!(p.system_ram_bytes, None);
    assert!(p.display.is_none());
    assert!(!p.reduced_motion);
}

#[test]
fn unknown_pointer_offers_no_hover() {
    // The conservative direction: until we know, assume a tooltip cannot be
    // reached, so the affordance carries a visible label.
    assert!(!PointerPrecision::Unknown.has_hover());
    assert!(!PointerPrecision::Unknown.has_touch());
}

#[test]
fn first_observation_sets_the_precision() {
    assert_eq!(
        PointerPrecision::Unknown.observe(PointerPrecision::Coarse),
        PointerPrecision::Coarse
    );
    assert_eq!(
        PointerPrecision::Unknown.observe(PointerPrecision::Fine),
        PointerPrecision::Fine
    );
}

#[test]
fn seeing_both_kinds_latches_to_both() {
    // The Android desktop case: a touchscreen and a mouse in one session. This
    // is the state Spec 08 §3.5 says features must handle, so it must be
    // reachable from either starting point.
    assert_eq!(
        PointerPrecision::Coarse.observe(PointerPrecision::Fine),
        PointerPrecision::Both
    );
    assert_eq!(
        PointerPrecision::Fine.observe(PointerPrecision::Coarse),
        PointerPrecision::Both
    );
}

#[test]
fn both_is_absorbing() {
    // Once a mouse has been seen, a later touch must not demote back to Coarse
    // and take the tooltips away.
    for seen in [
        PointerPrecision::Fine,
        PointerPrecision::Coarse,
        PointerPrecision::Both,
        PointerPrecision::Unknown,
    ] {
        assert_eq!(PointerPrecision::Both.observe(seen), PointerPrecision::Both);
    }
}

#[test]
fn repeated_observations_are_stable() {
    assert_eq!(
        PointerPrecision::Fine.observe(PointerPrecision::Fine),
        PointerPrecision::Fine
    );
    assert_eq!(
        PointerPrecision::Coarse.observe(PointerPrecision::Coarse),
        PointerPrecision::Coarse
    );
}

#[test]
fn an_unknown_observation_never_downgrades() {
    assert_eq!(
        PointerPrecision::Fine.observe(PointerPrecision::Unknown),
        PointerPrecision::Fine
    );
}

#[test]
fn both_has_hover_and_touch() {
    assert!(PointerPrecision::Both.has_hover());
    assert!(PointerPrecision::Both.has_touch());
    assert!(PointerPrecision::Fine.has_hover());
    assert!(!PointerPrecision::Fine.has_touch());
    assert!(PointerPrecision::Coarse.has_touch());
    assert!(!PointerPrecision::Coarse.has_hover());
}

#[test]
fn only_real_gpus_are_hardware_accelerated() {
    // The emulator case that `--cfg android_gpu` currently encodes at build
    // time: SwiftShader must not be handed the Vello compute path.
    assert!(GpuClass::Discrete.is_hardware_accelerated());
    assert!(GpuClass::Integrated.is_hardware_accelerated());
    assert!(!GpuClass::Software.is_hardware_accelerated());
    assert!(!GpuClass::None.is_hardware_accelerated());
    assert!(!GpuClass::Unknown.is_hardware_accelerated());
}

/// **The two questions must disagree, and `Software` is where.** A single
/// predicate answered both, and the budget — its only caller — read the
/// acceleration answer as a memory answer: a software adapter paints and
/// allocates page textures exactly like a hardware one, so treating it as
/// "no textures" collapsed the budget to its floor and softened visible body
/// text on every VM and headless desktop.
///
/// Asserted as the disagreement rather than as two separate tables, because a
/// table of each would both pass if the two functions were ever collapsed back
/// into one.
#[test]
fn a_software_rasteriser_allocates_textures_even_though_it_is_not_accelerated() {
    assert!(
        !GpuClass::Software.is_hardware_accelerated(),
        "Software must stay off the fast path",
    );
    assert!(
        GpuClass::Software.allocates_page_textures(),
        "Software still runs the paint path and still allocates page textures — \
         answering this with the acceleration predicate is what put the budget \
         on its 24 MiB floor on every machine wgpu gave llvmpipe",
    );
}

/// The polarity: `allocates_page_textures` is not simply `true`. Without an
/// adapter there is no paint path and no page textures.
#[test]
fn no_adapter_allocates_no_page_textures() {
    assert!(!GpuClass::None.allocates_page_textures());
}

#[test]
fn a_synthetic_profile_can_describe_an_android_desktop() {
    // R12's mitigation: the device we cannot buy, constructed in a test.
    let p = DeviceProfile {
        pointer: PointerPrecision::Both,
        system_ram_bytes: Some(16 * 1024 * 1024 * 1024),
        available_ram_bytes: Some(11 * 1024 * 1024 * 1024),
        gpu_class: GpuClass::Integrated,
        // An Android laptop driving a 2x panel — the case that makes R27 matter,
        // since residency goes as the square of this (Spec 08 §3.2a).
        device_scale_factor: Some(2.0),
        display: None,
        display_is_calibrated: false,
        window_mode: WindowMode::Windowed,
        reduced_motion: false,
    };
    assert!(p.pointer.has_hover(), "a mouse is attached: tooltips work");
    assert!(p.pointer.has_touch(), "the touchscreen still exists");
    assert!(p.gpu_class.is_hardware_accelerated());
    assert_eq!(p.window_mode, WindowMode::Windowed);
}
