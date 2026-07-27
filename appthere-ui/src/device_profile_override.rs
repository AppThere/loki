// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Forcing [`DeviceProfile`] fields, so a path can be exercised on hardware that
//! does not naturally produce it (Spec 08 T4.0).
//!
//! # Why this exists
//!
//! Phase 2 learned this the expensive way. Its screen session was going to report
//! the texture-pressure path clear on a machine where that path never engaged —
//! "scrolled around, looked fine" is not evidence about a branch that did not
//! run. The remedy was `LOKI_TEXTURE_BUDGET_MB`, which forces the *derived* value
//! so the branch can be reached deliberately.
//!
//! Phase 4 has the same problem one level up, and more of it. A development
//! machine with a trackpad and no touchscreen reports [`PointerPrecision::Fine`]
//! and never leaves it, so T4.3's tooltip path is exercised on every run and its
//! long-press-or-visible-label path is exercised on none. A session would report
//! Phase 4 clear on the strength of a branch that never ran — the identical
//! failure, in a place where nobody had thought to look for it.
//!
//! So the override moves from one derived number to the *inputs*, and serves
//! every consumer rather than one:
//!
//! | consumer | field | branch that would otherwise never run |
//! | --- | --- | --- |
//! | T4.3 tooltips | `pointer` | long-press / visible label on coarse pointers |
//! | T4.4 breakpoints | `viewport width` (via the window) | Compact and Medium size classes on a large display |
//! | T5.5 Actual Size | `display.px_per_inch` | the calibrated path, and the uncalibrated prompt |
//! | T7.1 status bar | `pointer`, `window_mode` | compact status-bar posture |
//!
//! # It forces, it does not fake
//!
//! An override writes the same fields a probe writes, through the same
//! `note_*` path, so every consumer downstream is exercised for real. Nothing
//! branches on "is this overridden" — that would make the overridden run a
//! different program from the real one, which is the failure this exists to
//! prevent rather than to introduce.
//!
//! # Read once per process
//!
//! Same reason as the texture budget: a profile field that changed under the
//! renderer mid-session would re-plan on a variable nobody expects to be live.
//! The probes that *are* live (memory, device scale factor) stay live — an
//! override simply wins whenever it is set.

use std::sync::OnceLock;

use crate::device_profile::{PhysicalDisplay, PointerPrecision, WindowMode};

/// Environment variable holding the overrides, as comma-separated `key=value`
/// pairs.
///
/// ```text
/// LOKI_DEVICE_PROFILE=pointer=coarse,ppi=96,window=fullscreen
/// ```
///
/// One variable rather than four because these are usually set together — a
/// coarse pointer on a windowed desktop is not a device anybody has — and a
/// single string keeps a test recipe copy-pasteable.
pub const OVERRIDE_ENV: &str = "LOKI_DEVICE_PROFILE";

/// Profile fields forced by the environment. `None` means "not overridden";
/// every field is independent.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ProfileOverride {
    /// Forces [`DeviceProfile::pointer`].
    pub pointer: Option<PointerPrecision>,
    /// Forces [`DeviceProfile::display`]'s pixels-per-inch.
    pub px_per_inch: Option<f32>,
    /// Forces [`DeviceProfile::window_mode`].
    pub window_mode: Option<WindowMode>,
    /// Forces [`DeviceProfile::device_scale_factor`].
    ///
    /// Included because the residency work found display scale to be the
    /// dominant axis of texture demand (Spec 08 R27), and a machine has exactly
    /// one of it. Overriding it lets the 3x and 4x rows of every table in §3.6
    /// be reached on a 2x display.
    pub device_scale_factor: Option<f64>,
}

impl ProfileOverride {
    /// `true` when nothing is forced — the ordinary case.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }

    /// The display override as a [`PhysicalDisplay`], if one was given.
    #[must_use]
    pub fn display(self) -> Option<PhysicalDisplay> {
        self.px_per_inch.map(|ppi| PhysicalDisplay {
            px_per_inch: Some(ppi),
        })
    }
}

/// Parses the override string. Unknown keys and unparseable values are ignored
/// rather than rejected.
///
/// Ignored, not rejected, for the same reason the budget override ignores a
/// typo: this is a diagnostic aid, and a mistyped variable must not stop the app
/// from starting. The cost is that a typo silently does nothing — which is why
/// [`describe`] exists and why the app logs what was applied.
#[must_use]
pub fn parse(raw: &str) -> ProfileOverride {
    let mut out = ProfileOverride::default();
    for pair in raw.split(',') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim().to_ascii_lowercase().as_str() {
            "pointer" => {
                out.pointer = match value.to_ascii_lowercase().as_str() {
                    "fine" => Some(PointerPrecision::Fine),
                    "coarse" => Some(PointerPrecision::Coarse),
                    "both" => Some(PointerPrecision::Both),
                    "unknown" => Some(PointerPrecision::Unknown),
                    _ => out.pointer,
                }
            }
            "ppi" => out.px_per_inch = value.parse().ok().filter(|v: &f32| *v > 0.0),
            "window" => {
                out.window_mode = match value.to_ascii_lowercase().as_str() {
                    "fullscreen" => Some(WindowMode::FullscreenSingle),
                    "windowed" => Some(WindowMode::Windowed),
                    _ => out.window_mode,
                }
            }
            "dsf" | "scale" => {
                out.device_scale_factor = value.parse().ok().filter(|v: &f64| *v > 0.0);
            }
            _ => {}
        }
    }
    out
}

/// The overrides in force, read once per process.
#[must_use]
pub fn current() -> ProfileOverride {
    static CACHED: OnceLock<ProfileOverride> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var(OVERRIDE_ENV)
            .ok()
            .map(|raw| parse(&raw))
            .unwrap_or_default()
    })
}

/// A one-line summary of what is forced, for the app to log at startup.
///
/// Returns `None` when nothing is overridden, so a caller can stay silent on an
/// ordinary run. **The app must log this when it is `Some`**: an override that
/// silently does nothing — a typo, a variable set in the wrong shell — would
/// make a session report a branch as exercised when it ran the default path,
/// which is precisely the failure the mechanism exists to prevent (L9-011).
#[must_use]
pub fn describe(o: ProfileOverride) -> Option<String> {
    if o.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = o.pointer {
        parts.push(format!("pointer={p:?}"));
    }
    if let Some(ppi) = o.px_per_inch {
        parts.push(format!("ppi={ppi}"));
    }
    if let Some(w) = o.window_mode {
        parts.push(format!("window={w:?}"));
    }
    if let Some(dsf) = o.device_scale_factor {
        parts.push(format!("dsf={dsf}"));
    }
    Some(parts.join(" "))
}

#[cfg(test)]
#[path = "device_profile_override_tests.rs"]
mod tests;
