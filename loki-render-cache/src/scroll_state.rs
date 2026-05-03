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
    /// Wall-clock time of the most recent scroll event. `None` until the
    /// first [`ScrollState::on_scroll`] call.
    last_event: Option<Instant>,
}

impl ScrollState {
    /// Creates a new [`ScrollState`] in the [`ScrollPhase::Idle`] phase with
    /// the viewport scrolled to the top.
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
    ///
    /// The timestamp is captured from the wall clock. Call
    /// [`ScrollState::tick`] on each frame to detect when the gesture settles.
    pub fn on_scroll(&mut self, new_top: f64) {
        self.on_scroll_at(new_top, Instant::now());
    }

    /// Internal timestamped variant used in tests to avoid real-time sleeps.
    fn on_scroll_at(&mut self, new_top: f64, now: Instant) {
        self.viewport_top_px = new_top;
        self.phase = ScrollPhase::Active;
        self.last_event = Some(now);
    }

    /// Advances the phase state machine.
    ///
    /// Returns `true` exactly once — on the frame where `phase` first
    /// transitions from [`ScrollPhase::Active`] to [`ScrollPhase::Settling`].
    /// Returns `false` on all subsequent calls until the next scroll gesture.
    ///
    /// Pass the current wall-clock time so callers can drive this
    /// deterministically in tests without sleeping.
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.phase != ScrollPhase::Active {
            return false;
        }
        let Some(last) = self.last_event else {
            return false;
        };
        if now.duration_since(last) >= SETTLE_DURATION {
            self.phase = ScrollPhase::Settling;
            return true;
        }
        false
    }

    /// Returns the half-open pixel range `[start, end)` of the *hot zone*:
    /// a region 2× the viewport height centred on the visible area.
    ///
    /// `start = viewport_top_px − 0.5 × viewport_height_px`
    /// `end   = viewport_top_px + 1.5 × viewport_height_px`
    ///
    /// Pages that overlap this range are candidates for full-resolution
    /// rendering.
    #[must_use]
    pub fn hot_range_px(&self) -> (f64, f64) {
        let half = self.viewport_height_px * 0.5;
        (
            self.viewport_top_px - half,
            self.viewport_top_px + self.viewport_height_px + half,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> ScrollState {
        ScrollState::new(800.0)
    }

    // ── tick behaviour ────────────────────────────────────────────────────────

    #[test]
    fn tick_before_settle_duration_returns_false_and_stays_active() {
        let mut state = make_state();
        let t0 = Instant::now();
        state.on_scroll_at(0.0, t0);

        let just_before = t0 + SETTLE_DURATION - Duration::from_millis(1);
        assert!(!state.tick(just_before));
        assert_eq!(state.phase, ScrollPhase::Active);
    }

    #[test]
    fn tick_after_settle_duration_returns_true_and_transitions_to_settling() {
        let mut state = make_state();
        let t0 = Instant::now();
        state.on_scroll_at(0.0, t0);

        let after = t0 + SETTLE_DURATION;
        assert!(state.tick(after));
        assert_eq!(state.phase, ScrollPhase::Settling);
    }

    #[test]
    fn second_tick_after_transition_returns_false_no_double_fire() {
        let mut state = make_state();
        let t0 = Instant::now();
        state.on_scroll_at(0.0, t0);

        let after = t0 + SETTLE_DURATION;
        assert!(state.tick(after));
        // Second call — phase is already Settling
        assert!(!state.tick(after + Duration::from_millis(10)));
        assert_eq!(state.phase, ScrollPhase::Settling);
    }

    #[test]
    fn tick_when_idle_returns_false() {
        let mut state = make_state();
        assert_eq!(state.phase, ScrollPhase::Idle);
        assert!(!state.tick(Instant::now()));
    }

    // ── hot_range_px ──────────────────────────────────────────────────────────

    #[test]
    fn hot_range_px_centred_on_viewport() {
        // viewport_top = 400, viewport_height = 800
        // half_vp = 400
        // expected: (400 - 400, 400 + 1200) = (0, 1600)
        let mut state = ScrollState::new(800.0);
        state.viewport_top_px = 400.0;

        let (start, end) = state.hot_range_px();
        assert_eq!(start, 0.0);
        assert_eq!(end, 1600.0);
    }

    #[test]
    fn hot_range_px_matches_spec_formula() {
        // Spec: (top - half_vp, top + 1.5 * vp_height)
        let top = 200.0_f64;
        let vp = 600.0_f64;
        let mut state = ScrollState::new(vp);
        state.viewport_top_px = top;

        let (start, end) = state.hot_range_px();
        assert!((start - (top - vp * 0.5)).abs() < f64::EPSILON);
        assert!((end - (top + vp * 1.5)).abs() < f64::EPSILON);
    }
}
