// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Zoom stepping and the presets, shared by every suite app's zoom control
//! (Spec 08 T5.4).
//!
//! # The sequence never wraps
//!
//! It used to: `next_zoom(200)` was `50`. That is defensible for a *badge* whose
//! only affordance is one click — the cycle is how you get back down. It is wrong
//! the moment there is a zoom **in** button, because "in" that lands further out
//! than where you started is not a zoom control, and a user who overshoots by one
//! press is thrown to the bottom of the range.
//!
//! So stepping saturates at both ends, and getting back down is [`prev_zoom`]'s
//! job. This is the requirement Spec 08 T5.4 states as "the increment sequence
//! never wraps", and it is why the old single-button caller had to gain a partner
//! rather than keep cycling.
//!
//! # Percent, not permille, at this layer
//!
//! The residency bound is in permille and `DocPageSource` compares against it,
//! because a zoom checked against a derived bound must compare exactly (Spec 08
//! T5.4 requirement 2). The *control* deals in whole percent, which is what a
//! user types and reads. The two meet at [`zoom_percent_to_permille`], which is
//! the one place the conversion happens.

/// The zoom presets a control offers, ascending, in whole percent.
///
/// Public because the preset list is both what the control renders and what the
/// stepper walks, and two copies would drift — an entry added to one and not the
/// other is invisible until someone counts.
pub const ZOOM_PRESETS_PERCENT: [u32; 9] = [25, 50, 75, 100, 125, 150, 200, 400, 600];

/// The lowest zoom the control offers, in percent (Spec 08 T5.4: 20–600%).
pub const ZOOM_MIN_PERCENT: u32 = 20;

/// The highest zoom the control offers, in percent.
///
/// What the **control offers**, explicitly not what the device can *serve* —
/// that is `appthere_canvas::residency::max_servable_zoom_permille`, and the
/// distinction is why the residency crate renamed its own constants to
/// `ZOOM_RANGE_*`. A control limited to the servable bound would silently differ
/// between machines; a control that clamped only to this value would exceed the
/// survival ceiling.
pub const ZOOM_MAX_PERCENT: u32 = 600;

/// The next preset above `current`, saturating at [`ZOOM_MAX_PERCENT`].
///
/// A value between presets steps to the next one above it, so a zoom arrived at
/// by typing or by Fit Width rejoins the sequence rather than jumping to a fixed
/// ladder position.
#[must_use]
pub fn next_zoom(current: u32) -> u32 {
    ZOOM_PRESETS_PERCENT
        .iter()
        .copied()
        .find(|&p| p > current)
        .unwrap_or(ZOOM_MAX_PERCENT)
        .min(ZOOM_MAX_PERCENT)
}

/// The next preset below `current`, saturating at [`ZOOM_MIN_PERCENT`].
#[must_use]
pub fn prev_zoom(current: u32) -> u32 {
    ZOOM_PRESETS_PERCENT
        .iter()
        .rev()
        .copied()
        .find(|&p| p < current)
        .unwrap_or(ZOOM_MIN_PERCENT)
        .max(ZOOM_MIN_PERCENT)
}

/// Clamps a typed or computed zoom to the range the control offers.
///
/// Total, so a field that cannot represent an out-of-range value cannot submit
/// one — the alternative being a validation message for a state the control
/// should not have been able to reach.
#[must_use]
pub fn clamp_zoom_percent(percent: u32) -> u32 {
    percent.clamp(ZOOM_MIN_PERCENT, ZOOM_MAX_PERCENT)
}

/// Parses a user-typed zoom, tolerating a trailing `%` and surrounding space.
///
/// `None` for anything that is not a number, so the caller can leave the field
/// alone rather than snapping it to a value the user did not ask for. A number
/// is returned clamped, which is how [`clamp_zoom_percent`]'s totality reaches
/// the keyboard.
#[must_use]
pub fn parse_zoom_percent(raw: &str) -> Option<u32> {
    let cleaned = raw.trim().trim_end_matches('%').trim();
    cleaned.parse::<u32>().ok().map(clamp_zoom_percent)
}

/// The control's percent as the permille the renderer and the residency bound
/// both speak.
///
/// The single conversion site. Spec 08 T5.4 requirement 2 exists because the
/// capability search once accumulated `zoom += 0.05` and landed on
/// 3.999999999999994 — unservable by 4e-15, which would have clamped every
/// device on ordinary paper. Integer percent × 10 cannot do that, and having
/// exactly one function do it means no caller can reintroduce it.
#[must_use]
pub fn zoom_percent_to_permille(percent: u32) -> u16 {
    u16::try_from(clamp_zoom_percent(percent).saturating_mul(10)).unwrap_or(u16::MAX)
}

/// Whether a capability limit is actually holding this zoom below what was
/// requested — the condition the control shows its "reduced" indicator for.
///
/// Spec 08 T5.4 requirement 6: the control displays the **requested** zoom, so
/// that adjusting from it does not erode the user's intent a nudge at a time.
/// The indicator is what stops that display from being a lie, and this predicate
/// is the whole of when it appears.
///
/// Strictly-less-than, so a limit that happens to equal the request is not
/// reported as a reduction: nothing has been taken away, and an indicator that
/// appears when nothing changed teaches the reader to ignore it.
#[must_use]
pub fn zoom_is_capped(requested_percent: u32, capability_limit_permille: Option<u16>) -> bool {
    match capability_limit_permille {
        Some(limit) => u32::from(limit) < u32::from(zoom_percent_to_permille(requested_percent)),
        None => false,
    }
}

#[path = "zoom_fit.rs"]
pub mod fit;

#[cfg(test)]
#[path = "zoom_tests.rs"]
mod tests;
