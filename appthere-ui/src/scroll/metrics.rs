// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`ScrollMetrics`] — the live geometry of a scroll container.

/// Live scroll geometry for a scroll container, mirrored from the most recent
/// DOM `scroll` event. All values are logical (CSS) pixels.
///
/// # `scroll_width` / `scroll_height` are distances, not sizes
///
/// The DOM `scroll` event Loki receives (PATCH(loki) in `dioxus-native-dom`)
/// reports Taffy geometry, where these two are the **scrollable distance**
/// (content − client), *not* the total content size. Total content size is
/// therefore `client + max_scroll`. Getting this backwards silently halves or
/// doubles every derived figure, so the accessors below are the intended way
/// to read it.
///
/// Defaults to all-zero, which callers treat as "not measured yet" — the first
/// scroll event (or the shell's post-resize replay) corrects it.
///
/// Moved here from `loki-text` so the scrollbar, the caret-follow controller,
/// and any future consumer read one type rather than three copies of six
/// `f32`s (Spec 08 S0.1 §3 — the same single-source rule Spec 01 audit A-1
/// applied to viewport width).
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct ScrollMetrics {
    /// Current vertical scroll offset.
    pub scroll_top: f32,
    /// Current horizontal scroll offset.
    pub scroll_left: f32,
    /// Horizontal scrollable **distance** (content width − client width).
    pub scroll_width: f32,
    /// Vertical scrollable **distance** (content height − client height).
    pub scroll_height: f32,
    /// Visible width of the container.
    pub client_width: f32,
    /// Visible height of the container.
    pub client_height: f32,
}

impl ScrollMetrics {
    /// `true` once a real scroll event has sized the container. An unmeasured
    /// container has no meaningful visible rect, so reveal requests against it
    /// are dropped rather than guessed.
    #[must_use]
    pub fn is_measured(&self) -> bool {
        self.client_height > 0.0 || self.client_width > 0.0
    }

    /// `true` when the content can be scrolled horizontally.
    #[must_use]
    pub fn can_scroll_x(&self) -> bool {
        self.client_width > 0.0 && self.scroll_width > 0.5
    }

    /// `true` when the content can be scrolled vertically.
    #[must_use]
    pub fn can_scroll_y(&self) -> bool {
        self.client_height > 0.0 && self.scroll_height > 0.5
    }

    /// Total content height in logical pixels (`client + max_scroll`).
    #[must_use]
    pub fn content_height(&self) -> f32 {
        self.client_height + self.scroll_height
    }

    /// Total content width in logical pixels (`client + max_scroll`).
    #[must_use]
    pub fn content_width(&self) -> f32 {
        self.client_width + self.scroll_width
    }

    /// The currently visible region in **content** coordinates, as
    /// `(x, y, width, height)`.
    ///
    /// This is the rect that `scroll_to_reveal` measures a target against: the
    /// caret's document position is in the same space, so "is the caret
    /// visible" is a plain containment test with no transform in between.
    #[must_use]
    pub fn visible_rect(&self) -> (f32, f32, f32, f32) {
        (
            self.scroll_left,
            self.scroll_top,
            self.client_width,
            self.client_height,
        )
    }
}

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
