// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Folding a [`crate::device_profile_override`] into the ambient profile.
//!
//! Split from `device_profile.rs` at the 300-line ceiling. The seam is natural:
//! everything here is about *forcing* a field, while its parent is about
//! observing one.

use dioxus::prelude::*;

use super::AtDeviceProfileContext;

/// Applies any [`crate::device_profile_override`] to the ambient profile.
///
/// Call once, from the app's device sensor, **before** the probes run — the
/// forced fields then stand and the unforced ones are probed normally.
///
/// Writes through the ordinary profile signal rather than through a parallel
/// path, so every consumer downstream is exercised for real. Nothing anywhere
/// branches on "is this overridden": an overridden run must be the same program
/// as a real one, or it tests something else.
pub fn apply_profile_override() {
    let o = crate::device_profile_override::current();
    if o.is_empty() {
        return;
    }
    let Some(ctx) = try_consume_context::<AtDeviceProfileContext>() else {
        return;
    };
    let mut profile = ctx.profile;
    let mut w = profile.write();
    if let Some(pointer) = o.pointer {
        w.pointer = pointer;
    }
    if let Some(display) = o.display() {
        w.display = Some(display);
    }
    if let Some(window_mode) = o.window_mode {
        w.window_mode = window_mode;
    }
    if let Some(dsf) = o.device_scale_factor {
        w.device_scale_factor = Some(dsf);
    }
}
