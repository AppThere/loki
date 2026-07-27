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

use appthere_ui::{GpuClass, note_device_scale_factor, note_gpu_class, use_memory_resampling};
use dioxus::prelude::*;
use loki_renderer::dpr_probe::observed_scale;
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
    // Overrides first, so a forced field stands and the probes fill in the rest.
    // Logged when non-empty: an override that silently did nothing — a typo, a
    // variable set in the wrong shell — would make a session report a branch as
    // exercised when it ran the default path, which is the failure the mechanism
    // exists to prevent (L9-011, and the reason the texture-budget procedure
    // needs two settings rather than one).
    use_hook(|| {
        appthere_ui::device_profile::apply_profile_override();
        if let Some(what) =
            appthere_ui::describe_device_profile_override(appthere_ui::device_profile_override())
        {
            tracing::info!(forced = %what, "device profile OVERRIDDEN");
        }
    });

    // Memory: seeded synchronously and then re-read on a cadence, per T1.6's
    // "observable, not sampled once".
    //
    // This was a bare `use_hook(|| note_system_memory(probe_system_memory()))`,
    // justified by "re-reading it per frame would make the budget jitter". The
    // objection to per-frame is right and the conclusion did not follow — those
    // are not the only two options — and once-at-mount undercut the reason the
    // derivation prefers `MemAvailable` over `MemTotal` at all: reacting to
    // pressure the app cannot see at startup. Launch is close to the worst
    // sampling moment, since this process is about to allocate.
    //
    // `use_memory_resampling` owns the cadence and the materiality grid.
    use_memory_resampling();

    // GPU: not answerable until the paint path has resumed. `note_gpu_class`
    // ignores `Unknown` and writes only on a change, so the common case — every
    // render after the first observation — writes nothing and wakes nobody.
    if let Some(kind) = observed_adapter() {
        note_gpu_class(gpu_class_of(kind));
    }

    // Display scale factor (R27): same shape and same reason as the GPU class —
    // Blitz owns it and only reveals it to the paint source, so it is observed on
    // the way past and lifted here. Unlike the adapter it can *change*
    // mid-session (a window dragged to another display), so this reads every
    // render rather than once; `note_device_scale_factor` writes only on a
    // change, so a stationary window wakes nobody.
    if let Some(scale) = observed_scale() {
        note_device_scale_factor(scale);
    }

    rsx! {}
}

#[cfg(test)]
#[path = "device_probe_tests.rs"]
mod tests;
