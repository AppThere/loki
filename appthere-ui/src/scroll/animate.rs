// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Smooth-scroll animation (Spec 08 T1.1).
//!
//! `MountedData::scroll` is instant regardless of the [`ScrollBehavior`] passed
//! — the vendored `dioxus-native-dom` patch performs the scroll eagerly and
//! ignores the flag, and `scroll_to` (`scrollIntoView`) is a documented no-op.
//! S0.1 identified animated programmatic scroll as the one scroll capability
//! Blitz does not provide; per that spike it is closed **app-side** rather than
//! with a patch, because the shell has no per-element animation clock we could
//! drive and the patch surface is better kept where it is.
//!
//! So: this module steps the offset itself, issuing a series of instant
//! scrolls. The easing curve is pure and unit-tested; the driver lives in
//! [`super::controller`].
//!
//! # Timing
//!
//! There is no `requestAnimationFrame` here and no async timer runtime under
//! `dioxus-native`. Ticks come from a worker thread that sleeps and signals
//! back through a channel — the same cross-thread yield the open-path layout
//! task and the save-status auto-clear already use. One thread per animation,
//! living ~200 ms; smooth scrolls are discrete user gestures (Find, Go To Page,
//! an outline click), never keystrokes, so they are rare by construction.

/// Wall-clock length of a smooth scroll. Short enough to feel like a response
/// rather than a transition — a caret reveal that takes longer than this reads
/// as lag, which is the failure mode T1.3 warns about.
pub(super) const SMOOTH_DURATION_MS: f32 = 180.0;

/// Interval between animation ticks, ≈60 Hz.
pub(super) const TICK_MS: u64 = 16;

/// Ease-out cubic: fast departure, gentle arrival.
///
/// Chosen over linear because a linear scroll stopping dead reads as a jump cut
/// at the end; ease-*out* specifically (rather than ease-in-out) because the
/// motion is a response to something the user just did, so it should start
/// immediately.
#[must_use]
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}

/// Position at `elapsed_ms` into a scroll from `from` to `to`, and whether the
/// animation is finished.
///
/// Finishing snaps exactly to `to`: interpolation alone would leave a
/// sub-pixel residue, and a scroll offset that never quite arrives keeps
/// `reveal_offset` asking for the same scroll forever.
#[must_use]
pub fn animation_step(from: f32, to: f32, elapsed_ms: f32, duration_ms: f32) -> (f32, bool) {
    if duration_ms <= 0.0 || elapsed_ms >= duration_ms {
        return (to, true);
    }
    let t = (elapsed_ms / duration_ms).clamp(0.0, 1.0);
    (from + (to - from) * ease_out_cubic(t), false)
}

/// Whether motion should be animated at all.
///
/// Not derived from a CSS media query: Stylo/Blitz expose no
/// `prefers-reduced-motion` to query, so this is carried explicitly and set
/// from the platform accessibility setting once `DeviceProfile` grows the probe
/// (`TODO(device-profile)` there). Defaulting to [`Self::Full`] keeps today's
/// behaviour; a user or platform that asks for reduced motion gets instant
/// scrolls, which is the correct degradation — the scroll still happens, it
/// just does not animate.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum MotionPreference {
    /// Animate smooth scrolls.
    #[default]
    Full,
    /// Perform every scroll instantly.
    Reduced,
}

impl MotionPreference {
    /// `true` when a smooth request should be honoured as an animation.
    #[must_use]
    pub fn animates(self) -> bool {
        self == Self::Full
    }
}

#[cfg(test)]
#[path = "animate_tests.rs"]
mod tests;
