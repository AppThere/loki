// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The measured viewport — the single source of truth for canvas placement and
//! window-size classification across the AppThere suite.

use super::breakpoint::Breakpoint;

/// Default logical DPI (CSS reference). `inner_width_px` is already in CSS px,
/// so this is a reference value for physical-size math (page-fit, Spec 03 M2),
/// not a scale applied to the width.
pub const DEFAULT_DPI: f32 = 96.0;

/// The measured viewport: the inner width of the document scroll container, plus
/// the zoom and DPI needed to reason about physical fit.
///
/// **One source of truth.** This replaces the former `window_width` signal,
/// which defaulted to 1280 px and was never written (Spec 01 audit A-1), so
/// hit-testing assumed a 1280-px window while rendered content used the real
/// measured width — the two diverged on every other window size. All canvas
/// origin / flex-centring math derives from this one measured value via
/// [`Viewport::centred_origin_x`].
///
/// **Relocated to `appthere_ui` (Spec 03 M1)** from `loki-text` so Presentation
/// and Spreadsheet share one viewport + breakpoint classification; it carries no
/// app-specific assumptions.
///
/// `inner_width_px` is `0.0` until the first measurement arrives; the centring
/// math clamps to `>= 0`, and an unmeasured viewport classifies as
/// [`Breakpoint::Compact`] (mobile-first, corrected on the first measured
/// frame). Page geometry stays per-document and is passed as the
/// `content_width_px` argument rather than living on this type.
///
/// `zoom` (1.0 = 100%) and `dpi` are carried for the Spec 03 M2 page-fit
/// renderer switch; the [`Breakpoint`] classification itself depends only on
/// `inner_width_px`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Measured inner width of the document scroll container, in CSS pixels.
    pub inner_width_px: f32,
    /// Zoom factor applied to document content; `1.0` = 100%.
    pub zoom: f32,
    /// Logical DPI for physical-size reasoning; see [`DEFAULT_DPI`].
    pub dpi: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            inner_width_px: 0.0,
            zoom: 1.0,
            dpi: DEFAULT_DPI,
        }
    }
}

impl Viewport {
    /// Builds a viewport from the measured scroll-container width (CSS px), with
    /// default zoom (100%) and DPI.
    #[must_use]
    pub fn new(inner_width_px: f32) -> Self {
        Self {
            inner_width_px,
            ..Self::default()
        }
    }

    /// Returns a copy with `zoom` set (1.0 = 100%).
    #[must_use]
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    /// Returns a copy with `dpi` set.
    #[must_use]
    pub fn with_dpi(mut self, dpi: f32) -> Self {
        self.dpi = dpi;
        self
    }

    /// The window-size class for this viewport's measured width (Spec 03 §5).
    #[must_use]
    pub fn breakpoint(&self) -> Breakpoint {
        Breakpoint::from_width(self.inner_width_px)
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
