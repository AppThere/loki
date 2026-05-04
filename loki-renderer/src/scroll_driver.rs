// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Dioxus signal side of scroll state.
//!
//! Two public helpers:
//!
//! - [`on_scroll_event`] — call from the document scroll handler to update the
//!   shared [`ScrollState`] signal.
//! - [`use_settle_detector`] — spawn once per document view; fires a callback
//!   exactly once per settle edge and resets the phase to [`ScrollPhase::Idle`].

use std::time::{Duration, Instant};

use dioxus::prelude::*;
use dioxus_core::Task;
use loki_render_cache::{ScrollPhase, ScrollState, SETTLE_DURATION};

// ── Settle poll interval ──────────────────────────────────────────────────────

/// How often the settle detector polls the scroll phase (≈60 fps).
const POLL_INTERVAL: Duration = Duration::from_millis(16);

// ── on_scroll_event ───────────────────────────────────────────────────────────

/// Call from the document scroll handler.
///
/// Updates `scroll.viewport_top_px` to `new_top_px` and transitions the phase
/// to [`ScrollPhase::Active`].  Thread-safe: Dioxus signals are always written
/// on the vdom thread.
pub fn on_scroll_event(mut scroll: Signal<ScrollState>, new_top_px: f64) {
    scroll.write().on_scroll(new_top_px);
}

// ── use_settle_detector ───────────────────────────────────────────────────────

/// Spawn once per document view.
///
/// Polls [`ScrollState::tick`] every [`POLL_INTERVAL`].  When the phase
/// transitions from [`ScrollPhase::Active`] to [`ScrollPhase::Settling`]
/// (`tick` returns `true`), fires `on_settle` **exactly once**, then resets
/// the phase to [`ScrollPhase::Idle`].
///
/// Returns a [`Task`] that can be cancelled on component unmount:
/// ```ignore
/// use_drop(move || settle_task.cancel());
/// ```
///
/// # Design
///
/// `use_settle_detector` must be called inside a Dioxus component or hook so
/// that `spawn` can register the task with the Dioxus async executor.  The
/// executor is single-threaded in Dioxus native, so `Signal<ScrollState>`
/// (which is `Copy + 'static`) is safely accessible from the spawned future.
pub fn use_settle_detector(
    mut scroll: Signal<ScrollState>,
    on_settle: impl Fn() + 'static,
) -> Task {
    spawn(async move {
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;

            // Tick advances the phase state machine.
            // Returns `true` exactly once on Active → Settling.
            let settled = scroll.write().tick(Instant::now());

            if settled {
                let top_px = scroll.read().viewport_top_px;
                tracing::info!(
                    viewport_top_px = top_px,
                    settle_duration_ms = SETTLE_DURATION.as_millis(),
                    "scroll settled",
                );
                on_settle();
                // Reset so the next scroll gesture starts clean.
                scroll.write().phase = ScrollPhase::Idle;
            }
        }
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// Test `ScrollState` transitions directly — no Dioxus runtime needed.
// Signal wiring is integration territory.

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use loki_render_cache::{ScrollPhase, ScrollState, SETTLE_DURATION};

    fn fresh_state() -> ScrollState {
        ScrollState::new(800.0)
    }

    // ── on_scroll behaviour (tested via ScrollState::on_scroll) ───────────────

    #[test]
    fn on_scroll_advances_viewport_top_and_activates() {
        let mut state = fresh_state();
        state.on_scroll(350.0);
        assert_eq!(state.viewport_top_px, 350.0);
        assert_eq!(state.phase, ScrollPhase::Active);
    }

    #[test]
    fn on_scroll_overwrites_previous_position() {
        let mut state = fresh_state();
        state.on_scroll(100.0);
        state.on_scroll(250.0);
        assert_eq!(state.viewport_top_px, 250.0);
        assert_eq!(state.phase, ScrollPhase::Active);
    }

    // ── settle detection (tested via ScrollState::tick) ───────────────────────

    #[test]
    fn tick_after_settle_duration_returns_true_and_transitions_to_settling() {
        let mut state = fresh_state();
        let t0 = Instant::now();
        state.on_scroll(0.0);
        // Use a time well past the settle threshold to avoid flakiness.
        let settled = state.tick(t0 + SETTLE_DURATION + Duration::from_secs(1));
        assert!(settled, "tick must return true on first Settling transition");
        assert_eq!(state.phase, ScrollPhase::Settling);
    }

    #[test]
    fn tick_does_not_fire_twice_for_same_gesture() {
        let mut state = fresh_state();
        let t0 = Instant::now();
        state.on_scroll(0.0);
        let future = t0 + SETTLE_DURATION + Duration::from_secs(1);
        assert!(state.tick(future));
        // Second tick — phase is already Settling, must return false.
        assert!(!state.tick(future + Duration::from_millis(16)));
    }

    // ── idle reset (mirrors what use_settle_detector does after on_settle) ────

    #[test]
    fn resetting_phase_to_idle_after_settling_clears_state() {
        let mut state = fresh_state();
        let t0 = Instant::now();
        state.on_scroll(100.0);
        state.tick(t0 + SETTLE_DURATION + Duration::from_secs(1));
        assert_eq!(state.phase, ScrollPhase::Settling);
        // The settle detector resets phase to Idle after calling on_settle.
        state.phase = ScrollPhase::Idle;
        assert_eq!(state.phase, ScrollPhase::Idle);
    }

    #[test]
    fn idle_state_tick_returns_false() {
        let mut state = fresh_state();
        assert_eq!(state.phase, ScrollPhase::Idle);
        assert!(!state.tick(Instant::now()));
    }
}
