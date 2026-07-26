// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Pure reveal geometry: what scroll offset brings a target rect into view.
//!
//! Separated from the Dioxus-facing controller so the arithmetic is unit-tested
//! headlessly — the same split `responsive::page_fit` and
//! `loki_renderer::virtualize` use.

/// How much clear space to keep around a revealed target, in logical pixels.
///
/// Named `leading` / `trailing` rather than `above` / `below` because the same
/// type serves both axes. For caret-follow the caller derives these from the
/// **live body-style line height**, not from a pixel constant (Spec 08 T1.3):
/// three lines of trailing space is a different number at 12 pt and at 24 pt,
/// and a hardcoded margin would be wrong at every zoom but one.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct RevealMargin {
    /// Space kept before the target — above it vertically, left of it
    /// horizontally.
    pub leading: f32,
    /// Space kept after the target — below it vertically, right of it
    /// horizontally.
    pub trailing: f32,
}

impl RevealMargin {
    /// A margin of `leading` and `trailing` logical pixels.
    #[must_use]
    pub fn new(leading: f32, trailing: f32) -> Self {
        Self { leading, trailing }
    }

    /// The caret-follow default expressed in lines: one line of clearance above
    /// and three below, so the next few lines the user is about to type are
    /// already on screen (Spec 08 T1.3).
    #[must_use]
    pub fn caret_lines(line_height_px: f32) -> Self {
        Self {
            leading: line_height_px,
            trailing: line_height_px * 3.0,
        }
    }
}

/// The scroll offset along one axis that reveals `[target_start, target_start +
/// target_len]` plus `margin`, or `None` when no scroll is needed.
///
/// All values are in **content** coordinates on that axis. `current` is the
/// present offset, `client` the visible extent, `max_scroll` the scrollable
/// distance (see [`super::ScrollMetrics`] — a distance, not a size).
///
/// Behaviour:
///
/// - Already fully visible, margins included → `None`. This is what keeps
///   typing in the middle of the page from scrolling at all.
/// - Target below the fold → scroll the **minimum** distance that brings its
///   trailing margin to the bottom edge.
/// - Target above the fold → scroll the minimum distance that brings its
///   leading margin to the top edge.
/// - The result is clamped to `[0, max_scroll]`, and a clamp that lands back on
///   `current` returns `None` rather than a no-op scroll — otherwise a caret on
///   the last line would re-issue a scroll on every keystroke forever.
///
/// When the target plus its margins is taller than the viewport, the target's
/// **leading** edge wins: the caret itself must stay visible even if the
/// trailing lines cannot.
#[must_use]
pub fn reveal_offset(
    current: f32,
    client: f32,
    max_scroll: f32,
    target_start: f32,
    target_len: f32,
    margin: RevealMargin,
) -> Option<f32> {
    if client <= 0.0 {
        return None; // unmeasured container — nothing to reveal into
    }

    let desired_min = target_start - margin.leading;
    let desired_max = target_start + target_len.max(0.0) + margin.trailing;

    let wanted = if desired_max - desired_min > client {
        // Cannot satisfy both edges. Anchor the leading edge so the target
        // itself is on screen; the trailing margin is the part we give up.
        desired_min
    } else if desired_max > current + client {
        desired_max - client
    } else if desired_min < current {
        desired_min
    } else {
        return None; // already visible with margins intact
    };

    let clamped = wanted.clamp(0.0, max_scroll.max(0.0));
    // Sub-pixel differences are not worth a scroll event; they also produce the
    // keystroke-loop described above.
    if (clamped - current).abs() < 0.5 {
        None
    } else {
        Some(clamped)
    }
}

#[cfg(test)]
#[path = "reveal_tests.rs"]
mod tests;
