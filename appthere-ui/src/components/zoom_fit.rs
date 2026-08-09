// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The three computed zooms — Fit Width, Fit Page, Actual Size (Spec 08 T5.4,
//! T5.13).
//!
//! # All three answer "what percent", so none of them is a mode
//!
//! A fit could be a persistent state that re-computes on every resize. It is not,
//! here: each of these produces a **zoom percent** which is then an ordinary
//! requested zoom like any other. That keeps one notion of "what zoom is in
//! effect", so the stepper, the field, the capability cap and the indicator all
//! keep working afterwards, and a user who picks Fit Width and then presses
//! zoom-in gets 'one preset above whatever that was' rather than a fight between
//! two owners of the same number.
//!
//! The cost is that Fit Width does not survive a window resize. That is the right
//! trade for a document editor — a fit that silently re-zoomed mid-edit is the
//! disruption Spec 08 T5.4 requirement 3 rules out for the capability bound, and
//! it would arrive here through the same door.
//!
//! # Actual Size is the one that can be *wrong* rather than merely unavailable
//!
//! Fit Width and Fit Page are computed from measurements the app already holds,
//! so they are always right. Actual Size depends on the display's physical
//! pixels-per-inch, which the platform may not report — and reporting a *wrong*
//! ppi produces a page that is confidently the wrong size, which is worse than
//! not offering the command. Hence [`actual_size_zoom_percent`] returns `Option`
//! and the caller must offer calibration rather than substituting a guess.

use super::clamp_zoom_percent;

/// CSS pixels per inch — the fixed ratio of the CSS pixel to the physical inch
/// that page geometry in points is converted through.
///
/// Not a display property: a CSS pixel is defined as 1/96 inch regardless of
/// what the panel actually does, which is exactly why Actual Size needs the
/// panel's *real* ppi separately.
const CSS_PX_PER_INCH: f32 = 96.0;

/// The zoom at which `page_width_px` fills `viewport_width_px`, less `margin_px`
/// of breathing room on both sides together.
///
/// `None` when either measurement is not yet real — a viewport of zero is what
/// the first frame reports, and a fit computed from it would be a division by
/// nothing dressed up as a zoom.
#[must_use]
pub fn fit_width_zoom_percent(
    viewport_width_px: f32,
    page_width_px_at_100: f32,
    margin_px: f32,
) -> Option<u32> {
    let usable = viewport_width_px - margin_px;
    if usable <= 1.0 || page_width_px_at_100 <= 1.0 {
        return None;
    }
    Some(clamp_zoom_percent(
        (usable / page_width_px_at_100 * 100.0).floor() as u32,
    ))
}

/// The zoom at which a whole page fits inside the viewport — the *smaller* of
/// the width and height fits, since a page that fits one axis and overflows the
/// other has not been fitted.
#[must_use]
pub fn fit_page_zoom_percent(
    viewport_width_px: f32,
    viewport_height_px: f32,
    page_width_px_at_100: f32,
    page_height_px_at_100: f32,
    margin_px: f32,
) -> Option<u32> {
    let by_width = fit_width_zoom_percent(viewport_width_px, page_width_px_at_100, margin_px)?;
    let by_height = fit_width_zoom_percent(viewport_height_px, page_height_px_at_100, margin_px)?;
    Some(by_width.min(by_height))
}

/// The zoom at which one document inch measures one physical inch on this
/// display.
///
/// `None` when the platform has not reported a physical pixel density — see the
/// module note. The caller offers per-display calibration instead, and only on
/// first use, so a reader who never asks for Actual Size is never asked to hold
/// a ruler to their screen.
///
/// # The arithmetic, stated because it is the part that is easy to get subtly wrong
///
/// A page is laid out in points and painted through a fixed 96 CSS px per inch.
/// At zoom 1.0 one document inch therefore occupies 96 CSS px, which occupies
/// `96 / css_px_per_inch` physical inches. To make that one physical inch, zoom
/// by `css_px_per_inch / 96`. The device *scale factor* does not enter: it maps
/// CSS px to device px, and `px_per_inch` here is already the CSS-pixel density,
/// which is what the platform physical-size query yields when divided by the
/// logical resolution.
#[must_use]
pub fn actual_size_zoom_percent(css_px_per_inch: Option<f32>) -> Option<u32> {
    let ppi = css_px_per_inch?;
    if !(ppi.is_finite() && ppi > 1.0) {
        return None;
    }
    Some(clamp_zoom_percent(
        (ppi / CSS_PX_PER_INCH * 100.0).round() as u32
    ))
}

#[cfg(test)]
#[path = "zoom_fit_tests.rs"]
mod tests;
