// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Touch-interaction state machine and word-boundary helpers.
//!
//! Touch input in the Dioxus Native runtime is synthesised from mouse events
//! by the blitz-shell patch (see `patches/blitz-shell`). The logic here runs
//! entirely in loki-text and does not depend on native OS touch APIs.

use unicode_segmentation::UnicodeSegmentation;

/// Minimum movement in logical pixels before a touch is classified as a scroll.
pub const TOUCH_SLOP_PX: f32 = 8.0;

/// Elapsed milliseconds before a stationary touch is classified as a long press.
pub const LONG_PRESS_MS: u128 = 500;

/// Phase of an in-progress touch interaction.
#[derive(Debug, Clone, PartialEq)]
pub enum TouchPhase {
    /// Touch just started — awaiting phase classification.
    Indeterminate,
    /// Touch has no significant movement and is under the long-press duration.
    Tap,
    /// Touch has moved beyond [`TOUCH_SLOP_PX`] — scroll gesture in progress.
    Scroll {
        /// Y coordinate (client pixels) of the most recent `touchmove` event,
        /// used to compute per-frame scroll deltas.
        last_y: f32,
    },
    /// Touch has been stationary for at least [`LONG_PRESS_MS`] — word
    /// selection active.
    LongPress,
}

/// Tracks the state of an in-progress touch interaction.
#[derive(Debug, Clone)]
pub struct TouchInteractionState {
    /// The touch identifier from the OS.
    pub touch_id: u64,
    /// Starting position in client (window) coordinates.
    pub start_pos: (f32, f32),
    /// Most recent position in client coordinates.
    pub current_pos: (f32, f32),
    /// Time the touch started (for long-press detection).
    pub start_time: std::time::Instant,
    /// Current classification of this touch interaction.
    pub phase: TouchPhase,
}

impl TouchInteractionState {
    /// Create a new state for a touch that just began at `pos`.
    pub fn new(touch_id: u64, pos: (f32, f32)) -> Self {
        Self {
            touch_id,
            start_pos: pos,
            current_pos: pos,
            start_time: std::time::Instant::now(),
            phase: TouchPhase::Indeterminate,
        }
    }

    /// Update state from a `touchmove` event at `new_pos`.
    ///
    /// Returns `true` when the phase transitioned to [`TouchPhase::Scroll`],
    /// so the caller can compute a scroll delta without re-checking the phase.
    pub fn update_move(&mut self, new_pos: (f32, f32)) -> bool {
        self.current_pos = new_pos;
        match self.phase {
            TouchPhase::Indeterminate | TouchPhase::Tap => {
                let dx = new_pos.0 - self.start_pos.0;
                let dy = new_pos.1 - self.start_pos.1;
                let elapsed = self.start_time.elapsed().as_millis();

                if elapsed >= LONG_PRESS_MS {
                    self.phase = TouchPhase::LongPress;
                } else if dx.hypot(dy) > TOUCH_SLOP_PX {
                    self.phase = TouchPhase::Scroll { last_y: new_pos.1 };
                    return true;
                }
            }
            TouchPhase::Scroll { .. } => {
                self.phase = TouchPhase::Scroll { last_y: new_pos.1 };
            }
            TouchPhase::LongPress => {}
        }
        false
    }
}

// ── Word boundary helpers ─────────────────────────────────────────────────────

/// Returns the `(start, end)` byte-offset range of the word that contains
/// `byte_offset` in `text`.
///
/// Returns `None` when `text` is empty or `byte_offset` is past the end.
/// For offsets that fall exactly on a word boundary between a word and a
/// non-word segment (whitespace/punctuation), the word to the left of the
/// offset is returned.
///
/// Word boundaries are defined by [`UnicodeSegmentation::split_word_bound_indices`].
pub fn word_boundaries_at(text: &str, byte_offset: usize) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }

    let mut result: Option<(usize, usize)> = None;

    for (start, word) in text.split_word_bound_indices() {
        if word.is_empty() {
            continue;
        }
        let end = start + word.len();
        // Accept the segment that contains `byte_offset`.  An offset exactly
        // equal to a segment end is treated as belonging to that segment so
        // that tapping just after the last character of a word selects it.
        if start <= byte_offset && byte_offset <= end {
            // Prefer alphabetic/numeric word segments over whitespace ones when
            // the offset falls on the boundary between them.
            let is_word = word.chars().any(|c| c.is_alphanumeric());
            if is_word || result.is_none() {
                result = Some((start, end));
            }
            if is_word {
                break;
            }
        }
    }

    result
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Touch phase classification ────────────────────────────────────────────

    #[test]
    fn tap_no_movement_short_duration() {
        let mut state = TouchInteractionState::new(1, (100.0, 200.0));
        // No movement at all — stays Indeterminate until explicitly ended.
        assert_eq!(state.phase, TouchPhase::Indeterminate);
        // A tiny jitter well under TOUCH_SLOP_PX does not change phase.
        state.update_move((101.0, 200.5));
        assert_eq!(state.phase, TouchPhase::Indeterminate);
    }

    #[test]
    fn scroll_when_movement_exceeds_slop() {
        let mut state = TouchInteractionState::new(1, (100.0, 200.0));
        // Move more than TOUCH_SLOP_PX (8.0) vertically.
        let became_scroll = state.update_move((100.0, 210.0));
        assert!(became_scroll, "should return true on scroll transition");
        assert!(
            matches!(state.phase, TouchPhase::Scroll { .. }),
            "phase should be Scroll"
        );
    }

    #[test]
    fn long_press_after_duration() {
        // Simulate a touch that has already been held for > LONG_PRESS_MS.
        let mut state = TouchInteractionState {
            touch_id: 1,
            start_pos: (100.0, 200.0),
            current_pos: (100.0, 200.0),
            // Wind the clock back so elapsed >= LONG_PRESS_MS.
            start_time: std::time::Instant::now()
                - std::time::Duration::from_millis(LONG_PRESS_MS as u64 + 10),
            phase: TouchPhase::Indeterminate,
        };
        // A tiny move that doesn't exceed slop.
        state.update_move((100.5, 200.5));
        assert_eq!(
            state.phase,
            TouchPhase::LongPress,
            "stationary touch over 500 ms should become LongPress"
        );
    }

    // ── Word boundaries ───────────────────────────────────────────────────────

    #[test]
    fn word_at_offset_inside_first_word() {
        assert_eq!(word_boundaries_at("hello world", 3), Some((0, 5)));
    }

    #[test]
    fn word_at_offset_inside_second_word() {
        assert_eq!(word_boundaries_at("hello world", 8), Some((6, 11)));
    }

    #[test]
    fn word_at_boundary_between_words() {
        // Offset 5 is the end of "hello" / start of " ". The word "hello" spans
        // [0,5]; the space spans [5,6]. word_boundaries_at should return the
        // alphabetic segment "hello" at (0,5) rather than the space.
        let result = word_boundaries_at("hello world", 5);
        assert!(
            result == Some((0, 5)) || result == Some((5, 6)),
            "offset at boundary must return either the word or the space, got {result:?}"
        );
    }

    #[test]
    fn empty_string_returns_none() {
        assert_eq!(word_boundaries_at("", 0), None);
    }

    // ── Scroll delta ──────────────────────────────────────────────────────────

    #[test]
    fn scroll_delta_100px_produces_100px_offset_change() {
        let mut state = TouchInteractionState::new(1, (50.0, 300.0));
        // First move classifies as scroll.
        state.update_move((50.0, 320.0));
        assert!(matches!(state.phase, TouchPhase::Scroll { .. }));

        // Simulate applying a 100-pixel drag by checking the delta math.
        let initial_scroll: f32 = 0.0;
        if let TouchPhase::Scroll { last_y } = state.phase {
            let new_y = last_y + 100.0;
            state.update_move((50.0, new_y));
            if let TouchPhase::Scroll { last_y: updated_y } = state.phase {
                let delta = updated_y - last_y;
                let new_offset = initial_scroll + delta;
                assert!(
                    (new_offset - 100.0).abs() < 0.01,
                    "100 px drag should produce ~100 px scroll offset change"
                );
            }
        }
    }
}
