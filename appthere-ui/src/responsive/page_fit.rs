// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Page-fit renderer decision (Spec 03 M2 / D2).
//!
//! The paginated ↔ non-paginated switch follows **content fit, not device
//! class**: use the paginated renderer when a full page column fits the
//! viewport at the current zoom, otherwise the reflow renderer to avoid
//! horizontal scrolling. A large landscape phone that fits a page gets
//! pagination; a narrow split-screen desktop window that can't gets reflow. The
//! [`Breakpoint`](crate::responsive::Breakpoint) still informs *chrome* posture
//! — the *renderer* follows this.
//!
//! The decision is **hysteretic**: a window dragged to exactly the boundary must
//! cross `threshold ± PAGE_FIT_HYSTERESIS_PX` to flip, so it never thrashes.
//!
//! All math is in CSS pixels, so it is DPI-independent; `viewport.dpi` is carried
//! for physical-size needs elsewhere but is not part of the fit comparison.

use super::viewport::Viewport;
use crate::tokens::layout::{PAGE_FIT_GUTTER_PX, PAGE_FIT_HYSTERESIS_PX};

/// The renderer posture the page-fit rule recommends.
///
/// Format-neutral (no dependency on any app's view-mode enum); consumers map it
/// onto their own renderer selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageFit {
    /// A page column fits — render paginated.
    Paginated,
    /// A page column does not fit — render reflowed to avoid horizontal scroll.
    Reflow,
}

/// The viewport width (CSS px) a page of `page_width_px` needs to display at the
/// viewport's zoom, including a gutter on each side.
#[must_use]
pub fn required_page_width(viewport: Viewport, page_width_px: f32) -> f32 {
    page_width_px * viewport.zoom + 2.0 * PAGE_FIT_GUTTER_PX
}

/// Whether the page fits *without* hysteresis — the bare predicate (use
/// [`resolve_page_fit`] for the renderer decision, which is sticky).
#[must_use]
pub fn page_fits(viewport: Viewport, page_width_px: f32) -> bool {
    viewport.inner_width_px >= required_page_width(viewport, page_width_px)
}

/// Resolves the renderer posture with hysteresis, given the *current* posture.
///
/// Switches to [`PageFit::Paginated`] only when the page fits with the full
/// hysteresis band to spare, and to [`PageFit::Reflow`] only when it overflows
/// by the band; within the `2×PAGE_FIT_HYSTERESIS_PX` dead-band the current
/// posture holds. An unmeasured viewport (`inner_width_px <= 0`) keeps the
/// current posture (nothing to decide yet).
#[must_use]
pub fn resolve_page_fit(
    viewport: Viewport,
    page_width_px: f32,
    currently_paginated: bool,
) -> PageFit {
    if viewport.inner_width_px <= 0.0 {
        return if currently_paginated {
            PageFit::Paginated
        } else {
            PageFit::Reflow
        };
    }
    let needed = required_page_width(viewport, page_width_px);
    let avail = viewport.inner_width_px;
    if currently_paginated {
        // Stay paginated until the page clearly overflows.
        if avail < needed - PAGE_FIT_HYSTERESIS_PX {
            PageFit::Reflow
        } else {
            PageFit::Paginated
        }
    } else {
        // Switch to paginated only when the page clearly fits.
        if avail >= needed + PAGE_FIT_HYSTERESIS_PX {
            PageFit::Paginated
        } else {
            PageFit::Reflow
        }
    }
}

#[cfg(test)]
#[path = "page_fit_tests.rs"]
mod tests;
