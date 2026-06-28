// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The measured editor viewport — the single source of truth for canvas
//! placement (Spec 01 audit A-1 / A-14).

/// The editor viewport: the measured inner width of the document scroll
/// container, in CSS pixels.
///
/// This replaces the former `window_width` signal, which **defaulted to 1280 px
/// and was never written** (Spec 01 audit A-1). Hit-testing therefore assumed a
/// 1280-px-wide window while the rendered / reflowed content used the *real*
/// measured width (`scroll_metrics.client_width`); the two diverged on every
/// window size other than 1280 px, drifting click-to-caret mapping. All canvas
/// origin / flex-centring math now derives from this one measured value via
/// [`Viewport::centred_origin_x`], which also replaces the centring formula that
/// was copy-pasted across the pointer handlers and the spelling panel (A-14).
///
/// `inner_width_px` is `0.0` until the first `get_client_rect` / scroll event
/// populates `scroll_metrics`; the centring math clamps to `>= 0`, so an
/// unmeasured viewport pins the canvas to the left edge for the single frame
/// before the first measurement arrives.
///
/// Page geometry stays in `DocumentState` (it is per-document) and is passed as
/// the `content_width_px` argument; zoom / DPI can join this type when Spec 03
/// (Responsive) needs them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
    /// Measured inner width of the document scroll container, in CSS pixels.
    pub inner_width_px: f32,
}

impl Viewport {
    /// Builds a viewport from the measured scroll-container width (CSS px).
    #[must_use]
    pub fn new(inner_width_px: f32) -> Self {
        Self { inner_width_px }
    }

    /// The left edge (CSS px) of an element of `content_width_px` that is
    /// flex-centred within the viewport — the document page (paginated) or the
    /// reflow tile (reflow). Clamped to `>= 0` so content wider than the
    /// viewport pins to the left rather than going negative.
    #[must_use]
    pub fn centred_origin_x(&self, content_width_px: f32) -> f32 {
        ((self.inner_width_px - content_width_px) / 2.0).max(0.0)
    }
}

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
