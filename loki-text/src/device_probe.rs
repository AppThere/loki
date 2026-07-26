// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Fills the ambient [`appthere_ui::DeviceProfile`] from the platform (Spec 08
//! T2.0).
//!
//! # Why the application does this and not either library
//!
//! The profile lives in `appthere-ui` (L5) and the GPU observation in
//! `loki-renderer` (L4); an edge between them would be uphill and the
//! dependency-direction gate refuses it. `loki-text` is L6 and depends on both,
//! so the join belongs here. It is also the layer that knows *this* application
//! paints with the GPU path at all — Calc and Slides will make the same call
//! when they grow one.
//!
//! # Two probes with different timing, and why only one is a sensor
//!
//! **Memory** is answerable at startup: a file read, done once on mount.
//!
//! **The GPU adapter is not.** Blitz picks it when the first page tile resumes,
//! which is after the editor has mounted — so at process start the honest
//! answer is `Unknown`, and a probe that returned a value then would be
//! guessing. This sensor therefore re-reads the observation cell on each render
//! it receives and folds in whatever is there, which is the "observable, not
//! sampled" shape `DeviceProfile` documents.
//!
//! **What that costs, stated plainly.** On a session where nothing ever
//! re-renders after the first paint, `gpu_class` stays `Unknown` until the next
//! interaction. That is acceptable *for this consumer specifically*: the
//! texture budget only binds once tiles are being painted, and painting tiles
//! means renders are happening. A consumer with different timing needs — say
//! one that must decide at launch — should not reuse this sensor without
//! checking that reasoning still holds for it.

use appthere_ui::{GpuClass, note_gpu_class, note_system_memory, probe_system_memory};
use dioxus::prelude::*;
use loki_renderer::gpu_probe::{AdapterKind, observed_adapter};

/// Maps the renderer's neutral adapter class onto the profile's capability
/// class.
///
/// `Virtual` maps to [`GpuClass::Integrated`] rather than `Software`: a
/// paravirtualised adapter is real hardware behind a passthrough layer and runs
/// Vello's compute pipelines. The emulator case that genuinely cannot —
/// SwiftShader — reports `Cpu`, and that is what must not be treated as capable
/// (S0.6 §3: the question the renderer asks is "can this device run Vello
/// compute", which an x86 emulator answers differently from a physical device).
#[must_use]
fn gpu_class_of(kind: AdapterKind) -> GpuClass {
    match kind {
        AdapterKind::Discrete => GpuClass::Discrete,
        AdapterKind::Integrated | AdapterKind::Virtual => GpuClass::Integrated,
        AdapterKind::Cpu => GpuClass::Software,
        // A backend that reports "other" has an adapter we cannot classify.
        // `Software` would be a claim we have not earned and `Discrete` a
        // dangerous one, so it reads as the conservative capable-but-modest
        // class, which is also what integrated hardware gets.
        AdapterKind::Other => GpuClass::Integrated,
    }
}

/// Zero-output sensor that keeps the device profile filled in.
///
/// Mounted once at the application root. Renders nothing.
#[component]
pub fn DeviceProbeSensor() -> Element {
    // Memory: answerable immediately, and it does not change during a session
    // in any way a budget should chase. `available` does move, but re-reading it
    // per frame would make the budget jitter with whatever else the machine is
    // doing — worse than a slightly stale figure.
    use_hook(|| note_system_memory(probe_system_memory()));

    // GPU: not answerable until the paint path has resumed. `note_gpu_class`
    // ignores `Unknown` and writes only on a change, so the common case — every
    // render after the first observation — writes nothing and wakes nobody.
    if let Some(kind) = observed_adapter() {
        note_gpu_class(gpu_class_of(kind));
    }

    rsx! {}
}

#[cfg(test)]
#[path = "device_probe_tests.rs"]
mod tests;
