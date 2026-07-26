// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Runtime device capabilities (Spec 08 T1.6, ADR L08-011).
//!
//! # Form factor is a runtime property
//!
//! An Android build may be running on a phone or on laptop-class hardware with
//! a desktop shell, a mouse, a hardware keyboard and desktop-tier RAM. So no
//! behaviour may be gated on `cfg!(target_os = ...)`; platform-specific *API
//! selection* is fine, platform-specific *behaviour* is not. Spike S0.6
//! enumerated the 11 sites that break that rule today
//! (`docs/spikes/S0.6-device-capability-probe.md` §2a); they migrate here.
//!
//! # This extends the responsive context, it does not replace it
//!
//! Viewport size and its [`Breakpoint`](crate::responsive::Breakpoint) already
//! live in [`crate::responsive`], measured from one source (Spec 01 audit A-1,
//! Spec 03 D4). Nothing here duplicates them — a consumer that wants "how wide
//! is the window" still reads the breakpoint. This carries the properties the
//! viewport cannot express: what is pointing at it, what is plugged into it,
//! and what it is made of.
//!
//! # Observable, not sampled
//!
//! A mouse can be plugged in mid-session; a window can move to another display.
//! The profile is a `Signal`, and consumers read the field they care about
//! through a memo so a change wakes only what it affects.
//!
//! # Injectable
//!
//! Probes are *supplied* to [`DeviceProfile`], never run inside it. That is
//! what lets Phases 2, 4, 5 and 7 be tested against synthetic profiles without
//! the hardware — the mitigation for Spec 08 R12, which is otherwise blocked on
//! owning an Android desktop device.

use dioxus::prelude::*;

/// What kind of pointing device is in use.
///
/// Both variants can be true at once: an Android desktop device with a
/// touchscreen and a mouse is [`Self::Both`], and features that key off this
/// must handle that rather than assuming a dichotomy (Spec 08 §3.5 — the
/// tooltip case).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PointerPrecision {
    /// Nothing has pointed at the app yet.
    #[default]
    Unknown,
    /// Mouse, trackpad or stylus — hover exists, small targets are reachable.
    Fine,
    /// Touch only — no hover, 44 px minimum targets, long-press replaces
    /// right-click and hover tooltips are unreachable.
    Coarse,
    /// Both are present in this session.
    Both,
}

impl PointerPrecision {
    /// `true` when a hover-triggered affordance (a tooltip) can actually be
    /// reached. False for [`Self::Unknown`]: until we know, assume the
    /// affordance needs a visible label, because an unreachable tooltip is a
    /// worse failure than a redundant label.
    #[must_use]
    pub fn has_hover(self) -> bool {
        matches!(self, Self::Fine | Self::Both)
    }

    /// `true` when touch input is available, so long-press and larger targets
    /// must be offered.
    #[must_use]
    pub fn has_touch(self) -> bool {
        matches!(self, Self::Coarse | Self::Both)
    }

    /// Folds an observed pointer event into the current state, latching to
    /// [`Self::Both`] once each kind has been seen.
    ///
    /// This is the interim signal S0.6 §4 describes: winit surfaces device
    /// add/remove but `blitz-shell` does not forward it yet, so precision is
    /// inferred from the events that do arrive. Once the shell forwards device
    /// enumeration this becomes a direct read and the latch can go.
    #[must_use]
    pub fn observe(self, seen: PointerPrecision) -> Self {
        match (self, seen) {
            (Self::Unknown, other) => other,
            (current, Self::Unknown) => current,
            (a, b) if a == b => a,
            _ => Self::Both,
        }
    }
}

/// Rough capability class of the GPU, from the wgpu adapter.
///
/// Replaces `cfg!(target_os = "android")` as the renderer-path selector: the
/// question the renderer actually asks is "can this device run Vello's compute
/// pipelines", which an emulator on x86 answers differently from a physical
/// Android device (S0.6 §2a, §3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GpuClass {
    /// Not yet probed.
    #[default]
    Unknown,
    /// Discrete GPU.
    Discrete,
    /// Integrated GPU.
    Integrated,
    /// Software rasteriser (SwiftShader, llvmpipe) — cannot run Vello compute.
    Software,
    /// No usable adapter; the CPU renderer is the only option.
    None,
}

impl GpuClass {
    /// `true` when the GPU paint path is viable.
    #[must_use]
    pub fn supports_gpu_paint(self) -> bool {
        matches!(self, Self::Discrete | Self::Integrated)
    }
}

/// Physical characteristics of the display a window is on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PhysicalDisplay {
    /// Measured or calibrated pixels per inch. `None` while unknown — per D-04
    /// the calibration prompt appears on first use of Actual Size, never at
    /// first run, so an unknown value is a normal state and not an error.
    pub px_per_inch: Option<f32>,
}

/// How the window is presented.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WindowMode {
    /// Not yet determined.
    #[default]
    Unknown,
    /// One window filling the display — the phone default, and desktop
    /// fullscreen.
    FullscreenSingle,
    /// A window among others, freely resizable.
    Windowed,
}

/// A snapshot of what this session is running on.
///
/// Construct with [`Self::default`] and fill in fields as probes report; every
/// field is independently `Unknown`/`None` until then, and consumers must
/// behave sensibly in that state rather than waiting for it.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
/// There is deliberately **no `hardware_keyboard`** field. It existed until r20
/// and was removed rather than left unwired.
///
/// S0.6 introduced it and §3.5 assigned it two consumers, I-05 and I-07. Both
/// collapsed for the same reason: the Android inset chain reports *actual* IME
/// visibility (`WindowInsets.Type.ime()` folded into the mask, re-queried on every
/// visibility change), so no consumer needs to ask whether a keyboard is attached —
/// with a hardware keyboard there is no IME, so the inset is 0 and nothing is
/// reserved, for free.
///
/// That left a `bool` nothing read, whose own doc comment forbade using it to
/// reserve space — its only intended purpose. An API that invites a use its
/// contract forbids is the trap L08-031 is about, and an unread field is the
/// version of that trap nobody notices until they reach for it. If a genuine
/// consumer appears — surfacing keyboard shortcuts only when a keyboard exists is
/// the plausible one — adding a probed `bool` back is a smaller change than
/// discovering this one was never wired.
pub struct DeviceProfile {
    /// What is pointing at the app.
    pub pointer: PointerPrecision,
    /// Total system RAM, when the platform has been queried.
    pub system_ram_bytes: Option<u64>,
    /// RAM the OS believes is available without swapping, when known.
    ///
    /// Prefer this over [`Self::system_ram_bytes`] when sizing a budget: total
    /// RAM says what the machine has, not what this process may take while a
    /// browser and the OS are also resident — which is the situation Spec 06's
    /// 8 GB design floor describes.
    pub available_ram_bytes: Option<u64>,
    /// GPU capability class.
    ///
    /// There is deliberately no `gpu_memory_bytes` beside it.
    /// TODO(device-profile-gpu-memory): wgpu exposes no portable VRAM figure —
    /// `AdapterInfo` carries a device type, vendor and backend but no capacity —
    /// so a budget derives from system RAM and this class instead. Recording the
    /// absence rather than inventing a number is the point: an integrated GPU
    /// shares system RAM (so the system figure *is* the constraint), and a
    /// discrete one has its own budget we cannot see.
    pub gpu_class: GpuClass,
    /// Physical pixels per CSS pixel on the display this window is on, when
    /// observed.
    ///
    /// `None` means no page tile has painted yet, and is deliberately distinct
    /// from `Some(1.0)`: a consumer must be able to tell an unprobed HiDPI
    /// display from a genuine standard-DPI one, because resident texture bytes go
    /// as the *square* of this number (Spec 08 §3.2a) and assuming 1.0 on a 3x
    /// display under-states the cost ninefold.
    ///
    /// Owned by the compositor rather than by us, so it is observed on the way
    /// past rather than queried — see `loki_renderer::dpr_probe` — and it can
    /// change mid-session when a window moves between displays.
    pub device_scale_factor: Option<f64>,
    /// The current display's physical characteristics.
    pub display: Option<PhysicalDisplay>,
    /// How the window is presented.
    pub window_mode: WindowMode,
    /// Whether the user or platform asked for reduced motion. Wired to
    /// [`crate::MotionPreference`] by the app.
    ///
    /// TODO(device-profile): probe the platform setting — Android
    /// `Settings.Global.ANIMATOR_DURATION_SCALE`, Windows
    /// `SPI_GETCLIENTAREAANIMATION`, macOS
    /// `accessibilityDisplayShouldReduceMotion`. Until then this is only ever
    /// set by an explicit user preference, and defaults to full motion.
    pub reduced_motion: bool,
}

/// The device-profile context injected at the application root.
#[derive(Clone, Copy, PartialEq)]
pub struct AtDeviceProfileContext {
    /// The live profile.
    pub profile: Signal<DeviceProfile>,
}

/// Provides [`AtDeviceProfileContext`] at the application root and returns the
/// backing signal so probes can push into it. Call once, in the root component.
pub fn use_provide_device_profile() -> Signal<DeviceProfile> {
    let profile = use_signal(DeviceProfile::default);
    provide_context(AtDeviceProfileContext { profile });
    profile
}

/// Reads the device profile injected at the application root.
///
/// Returns [`DeviceProfile::default`] — everything `Unknown` — when no context
/// has been provided, so a component used outside an app root (a test, a
/// preview) degrades instead of panicking.
#[must_use]
pub fn use_device_profile() -> DeviceProfile {
    match try_consume_context::<AtDeviceProfileContext>() {
        Some(ctx) => *ctx.profile.read(),
        None => DeviceProfile::default(),
    }
}

/// Folds an observed pointer kind into the ambient profile.
///
/// Cheap enough to call from every pointer handler: it writes only when the
/// precision actually changes, so a stream of mouse-moves does not wake the
/// consumers of the signal.
pub fn note_pointer(seen: PointerPrecision) {
    let Some(ctx) = try_consume_context::<AtDeviceProfileContext>() else {
        return;
    };
    let mut profile = ctx.profile;
    let current = profile.peek().pointer;
    let next = current.observe(seen);
    if next != current {
        profile.write().pointer = next;
    }
}

#[cfg(test)]
#[path = "device_profile_tests.rs"]
mod tests;
