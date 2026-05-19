// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Scroll phase tracking and hot-zone geometry.

use std::time::{Duration, Instant};

/// Scroll must be idle for this long before the phase transitions to
/// [`ScrollPhase::Settling`].
pub const SETTLE_DURATION: Duration = Duration::from_millis(120);

/// The lifecycle phase of the current scroll gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPhase {
    /// Scroll events are arriving; the viewport is moving.
    Active,
    /// No new scroll event has arrived for [`SETTLE_DURATION`]; the cache
    /// should begin promoting pages to higher-resolution tiers.
    Settling,
    /// Fully quiescent — no in-flight scroll gesture.
    Idle,
}

/// Tracks scroll position and phase, and computes the hot cache zone.
#[derive(Debug)]
pub struct ScrollState {
    /// Current scroll phase.
    pub phase: ScrollPhase,
    /// Y coordinate of the top edge of the visible area, in logical pixels.
    pub viewport_top_px: f64,
    /// Height of the visible area, in logical pixels.
    pub viewport_height_px: f64,
    /// Wall-clock time of the most recent scroll event.
    last_event: Option<Instant>,
}

impl ScrollState {
    /// Creates a new [`ScrollState`] in the [`ScrollPhase::Idle`] phase.
    #[must_use]
    pub fn new(viewport_height_px: f64) -> Self {
        Self {
            phase: ScrollPhase::Idle,
            viewport_top_px: 0.0,
            viewport_height_px,
            last_event: None,
        }
    }

    /// Records a new scroll position, transitioning to [`ScrollPhase::Active`].
    pub fn on_scroll(&mut self, new_top: f64) {
        self.viewport_top_px = new_top;
        self.phase = ScrollPhase::Active;
        self.last_event = Some(Instant::now());
    }

    /// Returns `(start, end)` of the hot zone: 2× viewport height centred on
    /// the visible area.
    #[must_use]
    pub fn hot_range_px(&self) -> (f64, f64) {
        let half = self.viewport_height_px * 0.5;
        (
            self.viewport_top_px - half,
            self.viewport_top_px + self.viewport_height_px + half,
        )
    }
}
