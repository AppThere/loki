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

use appthere_ui::{
    GpuClass, note_device_scale_factor, note_display_density, note_gpu_class, use_memory_resampling,
};
use dioxus::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use loki_renderer::dpr_probe::observed_scale;

/// Whether the display-density probe has already run this process.
static DENSITY_PROBED: AtomicBool = AtomicBool::new(false);
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

    // **The paint path's answers arrive with no signal, so they are polled.**
    //
    // Everything below reads a value the paint path publishes *later* — the GPU
    // adapter and the display scale factor are both `None` until a page tile has
    // painted. This component had no reactive read at all, so it rendered exactly
    // once, at startup, with both reads `None`; the GPU class and the scale
    // factor could therefore **never** reach the profile, and the comment
    // claiming this "reads every render" described an intent rather than a
    // behaviour (Spec 08 r81 — the capability-bound-with-no-caller shape again).
    //
    // Subscribing to the profile was the first fix and is not enough: the memory
    // probe writes only on a *change*, so on a machine whose memory is steady
    // nothing ever re-renders this. `observed_scale` and `observed_adapter` are
    // plain statics with no change notification, so a poll is the honest tool.
    //
    // It is bounded and self-terminating: it stops as soon as both are known, so
    // the steady state is no thread at all rather than a timer nobody notices.
    use_paint_observation_lifting();

    rsx! {}
}

#[cfg(test)]
#[path = "device_probe_tests.rs"]
mod tests;

/// The platform display-density query for this build.
///
/// X11 only today. The other three surfaces T5.5 names — Wayland `wl_output`,
/// macOS `CGDisplayScreenSize`, Windows EDID — are **not written**, and are
/// absent rather than stubbed so that "no answer" reaches the caller identically
/// however it arose. That matters because the caller's response is the same in
/// every case: offer calibration.
#[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
fn probe_display_density(
    device_scale_factor: f64,
) -> Option<loki_app_shell::display_density::DisplayDensity> {
    loki_app_shell::display_probe_x11::probe_css_ppi(device_scale_factor)
}

/// The no-query build. The parameter is named `_device_scale_factor` rather
/// than discarded in the body, so the platform that has no probe reads as a
/// platform that has no probe — not as one whose probe ignores its input.
#[cfg(not(all(unix, not(target_os = "macos"), not(target_os = "android"))))]
fn probe_display_density(
    _device_scale_factor: f64,
) -> Option<loki_app_shell::display_density::DisplayDensity> {
    None
}

/// How often to look for the paint path's observations, and for how long.
///
/// 250 ms is well inside a reader's reaction time to the first frame, and the
/// cap stops a session that never paints — an app left on the Home screen — from
/// keeping a thread alive for its whole life. Reaching the cap is not an error:
/// nothing has painted, so there is nothing to observe and no consumer waiting.
const OBSERVATION_POLL_MS: u64 = 250;
const OBSERVATION_POLL_LIMIT: u32 = 240; // one minute

/// Lifts the paint path's observations — GPU class, display scale factor, and
/// the display density that depends on the scale — into the device profile.
///
/// Spawned once. The poll thread only reads statics; every profile write happens
/// on the runtime, where the Dioxus context lives.
fn use_paint_observation_lifting() {
    use_hook(|| {
        let (tx, mut rx) = futures_channel::mpsc::unbounded::<(Option<AdapterKind>, Option<f64>)>();
        let spawned = std::thread::Builder::new()
            .name("loki-paint-observation".into())
            .spawn(move || {
                for _ in 0..OBSERVATION_POLL_LIMIT {
                    std::thread::sleep(Duration::from_millis(OBSERVATION_POLL_MS));
                    let seen = (observed_adapter(), observed_scale());
                    if seen == (None, None) {
                        continue;
                    }
                    // A closed channel means the runtime is gone.
                    if tx.unbounded_send(seen).is_err() {
                        return;
                    }
                    if seen.0.is_some() && seen.1.is_some() {
                        return;
                    }
                }
            });
        if spawned.is_ok() {
            spawn(async move {
                use futures_util::StreamExt;
                while let Some((adapter, scale)) = rx.next().await {
                    if let Some(kind) = adapter {
                        note_gpu_class(gpu_class_of(kind));
                    }
                    let Some(scale) = scale else { continue };
                    note_device_scale_factor(scale);
                    if DENSITY_PROBED.swap(true, Ordering::Relaxed) {
                        continue;
                    }
                    match probe_display_density(scale) {
                        Some(d) => {
                            tracing::info!(
                                css_px_per_inch = d.css_px_per_inch,
                                device_scale_factor = scale,
                                "display density probed — Actual Size is available",
                            );
                            note_display_density(d.css_px_per_inch, false);
                        }
                        // Logged rather than silent: "Actual Size is missing" is
                        // otherwise indistinguishable from "the row was never
                        // wired", which is the confusion that made the first
                        // screen sitting necessary at all.
                        None => tracing::info!(
                            "no believable display density — Actual Size stays \
                             hidden until the reader calibrates",
                        ),
                    }
                }
            });
        }
    });
}
