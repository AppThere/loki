// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Which offset a command should compose against (Spec 08 T5.6).
//!
//! # The metrics signal reports the past
//!
//! [`ScrollMetrics`] mirrors the DOM's `onscroll` event, so it says where the
//! container *has been observed* to be — one event loop behind a scroll the
//! controller just issued. One command per interaction never notices. A gesture
//! does: X11 delivers two wheel events per notch and a trackpad delivers a
//! stream of them, so the second command reads the offset from before the first
//! had moved anything, and the error compounds across the burst.
//!
//! The sitting measured this directly rather than inferring it — two `zoom_to`
//! calls 0.2 ms apart, both reading `scroll_top = 200.0`, both computing the
//! same target. The zoom advanced twice; the scroll moved once, and the anchored
//! point slid by the difference.
//!
//! # Self-clearing, because a remembered position that outlives its reason lies
//!
//! A command records what it asked for *together with the metrics it asked
//! against*. [`effective_offset`] returns the remembered offset while those
//! metrics are still what the DOM reports, and the reported offset the moment
//! they are not. No timer, no subscription, and nothing to reset — one scroll
//! event and the memory is gone.
//!
//! # What this is not
//!
//! Not a model of where the container is. If the container clamps a command — a
//! scroll past the end — this returns the unclamped value until the DOM reports
//! otherwise. That error is bounded by the clamp distance and lasts one event,
//! where reading the stale metrics is an error that compounds with every event
//! in the burst.

use super::metrics::ScrollMetrics;

/// A scroll offset that was commanded, and the metrics it was computed against.
pub(super) type Commanded = (f32, f32, ScrollMetrics);

/// The offset a new command should compose against.
///
/// `commanded` is the last command issued, or `None` if none has been.
#[must_use]
pub(super) fn effective_offset(metrics: ScrollMetrics, commanded: Option<Commanded>) -> (f32, f32) {
    match commanded {
        Some((x, y, against)) if against == metrics => (x, y),
        _ => (metrics.scroll_left, metrics.scroll_top),
    }
}

#[cfg(test)]
#[path = "controller_offset_tests.rs"]
mod tests;
