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
fn only_real_gpus_support_the_paint_path() {
    // The emulator case that `--cfg android_gpu` currently encodes at build
    // time: SwiftShader must not be handed the Vello compute path.
    assert!(GpuClass::Discrete.supports_gpu_paint());
    assert!(GpuClass::Integrated.supports_gpu_paint());
    assert!(!GpuClass::Software.supports_gpu_paint());
    assert!(!GpuClass::None.supports_gpu_paint());
    assert!(!GpuClass::Unknown.supports_gpu_paint());
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
        window_mode: WindowMode::Windowed,
        reduced_motion: false,
    };
    assert!(p.pointer.has_hover(), "a mouse is attached: tooltips work");
    assert!(p.pointer.has_touch(), "the touchscreen still exists");
    assert!(p.gpu_class.supports_gpu_paint());
    assert_eq!(p.window_mode, WindowMode::Windowed);
}
