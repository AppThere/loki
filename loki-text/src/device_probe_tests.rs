// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the adapter-class mapping. Extracted per the file-ceiling idiom.
//!
//! The sensor component itself needs a Dioxus runtime and a provided profile
//! context, so what is pinned here is the mapping — which is where the
//! judgement calls are, and the only part a reader could get wrong.

use super::gpu_class_of;
use appthere_ui::GpuClass;
use loki_renderer::gpu_probe::AdapterKind;

#[test]
fn real_hardware_is_capable_and_a_software_rasteriser_is_not() {
    assert!(gpu_class_of(AdapterKind::Discrete).is_hardware_accelerated());
    assert!(gpu_class_of(AdapterKind::Integrated).is_hardware_accelerated());
    // The one that must be false: SwiftShader on the Android emulator reports
    // `Cpu`, and it cannot run Vello's compute pipelines.
    assert!(!gpu_class_of(AdapterKind::Cpu).is_hardware_accelerated());
    assert_eq!(gpu_class_of(AdapterKind::Cpu), GpuClass::Software);
}

#[test]
fn a_paravirtualised_adapter_counts_as_hardware() {
    // A VM guest with GPU passthrough is real hardware behind a shim. Treating
    // it as software would drop a capable device onto the CPU renderer — and
    // "runs in a VM" is how a lot of Android desktop images present.
    assert_eq!(gpu_class_of(AdapterKind::Virtual), GpuClass::Integrated);
}

#[test]
fn an_unclassifiable_adapter_gets_the_conservative_capable_class() {
    // Not `Software` (a claim we have not earned) and not `Discrete` (one that
    // would inflate a budget).
    assert_eq!(gpu_class_of(AdapterKind::Other), GpuClass::Integrated);
}
