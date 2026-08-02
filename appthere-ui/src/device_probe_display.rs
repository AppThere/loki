// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The display-density setter (Spec 08 T5.5). Split from `device_probe.rs` at
//! the 300-line ceiling.

use dioxus::prelude::*;

use super::AtDeviceProfileContext;
use crate::device_profile::PhysicalDisplay;

/// Records the display's CSS pixels-per-inch (Spec 08 T5.5).
///
/// # An override wins, and a *worse* source never overwrites a better one
///
/// Two rules, and the second is the one that is easy to miss. A forced `ppi`
/// wins for the reason the scale-factor override does: the probe would otherwise
/// overwrite it and the branch the override exists to reach would never run.
///
/// The second rule is that a reader who calibrated this display must not have
/// their measurement replaced by the platform's guess on the next resize — the
/// calibration *is* the correction to that guess. So a `Platform` reading is
/// ignored once a `Calibrated` one is in place, and only a fresh calibration can
/// replace a calibration. Without that ordering the prompt would appear to work
/// and then quietly undo itself, which is worse than not offering it.
pub fn note_display_density(css_px_per_inch: f32, calibrated: bool) {
    if crate::device_profile_override::current()
        .css_px_per_inch
        .is_some()
    {
        return;
    }
    if !css_px_per_inch.is_finite() || css_px_per_inch <= 0.0 {
        return;
    }
    let Some(ctx) = try_consume_context::<AtDeviceProfileContext>() else {
        return;
    };
    let mut profile = ctx.profile;
    {
        let current = profile.peek();
        if current.display_is_calibrated && !calibrated {
            return;
        }
        if current.display.and_then(|d| d.css_px_per_inch) == Some(css_px_per_inch)
            && current.display_is_calibrated == calibrated
        {
            return;
        }
    }
    let mut w = profile.write();
    w.display = Some(PhysicalDisplay {
        css_px_per_inch: Some(css_px_per_inch),
    });
    w.display_is_calibrated = calibrated;
}
