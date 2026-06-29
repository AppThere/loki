// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::{resolve_page_fit, PageFit};
use crate::responsive::Viewport;

/// Default A4 page width (CSS px), as `DocumentState` seeds it.
const PAGE: f32 = 794.0;

fn vp(width: f32) -> Viewport {
    Viewport::new(width)
}

// Spec 03 M2 acceptance — window-free page-fit decisions (Spec 02 harness style).

#[test]
fn landscape_phone_that_fits_a_page_renders_paginated() {
    // ~iPhone Pro Max landscape (926 px) clears the page + gutter + hysteresis.
    assert_eq!(resolve_page_fit(vp(926.0), PAGE, false), PageFit::Paginated);
}

#[test]
fn narrow_desktop_window_that_cannot_fit_a_page_renders_reflow() {
    // A 700-px split-screen desktop window can't fit a 794-px page.
    assert_eq!(resolve_page_fit(vp(700.0), PAGE, true), PageFit::Reflow);
}

#[test]
fn dragging_across_the_boundary_does_not_thrash() {
    // Walk the width down through the dead-band, then back up, carrying the
    // resolved mode forward as the "current" posture each step (as the editor
    // does). The mode must flip at most once in each direction, and only at the
    // band edges — never oscillate within the band.
    let widths_down: Vec<f32> = (700..=1000).rev().map(|w| w as f32).collect();
    let mut paginated = true; // start wide & paginated
    let mut transitions = 0;
    for w in widths_down {
        let next = resolve_page_fit(vp(w), PAGE, paginated) == PageFit::Paginated;
        if next != paginated {
            transitions += 1;
            // Going narrower, the only allowed flip is paginated→reflow.
            assert!(paginated && !next, "unexpected flip at width {w}");
        }
        paginated = next;
    }
    assert_eq!(transitions, 1, "should flip exactly once dragging narrower");
    assert!(!paginated, "narrowest width must end reflowed");

    // Now widen back up.
    let mut transitions_up = 0;
    for w in 700..=1000 {
        let next = resolve_page_fit(vp(w as f32), PAGE, paginated) == PageFit::Paginated;
        if next != paginated {
            transitions_up += 1;
            assert!(!paginated && next, "unexpected flip at width {w}");
        }
        paginated = next;
    }
    assert_eq!(transitions_up, 1, "should flip exactly once dragging wider");
    assert!(paginated, "widest width must end paginated");
}

#[test]
fn dead_band_holds_whichever_mode_entered_it() {
    // 842 px = exactly page + gutters (needed). Inside [794, 890) the decision is
    // sticky: the same width yields different results depending on entry mode.
    assert_eq!(resolve_page_fit(vp(842.0), PAGE, true), PageFit::Paginated);
    assert_eq!(resolve_page_fit(vp(842.0), PAGE, false), PageFit::Reflow);
}

#[test]
fn zoom_scales_the_page_so_a_fitting_window_can_stop_fitting() {
    // 1000 px fits the page at 100% …
    assert_eq!(
        resolve_page_fit(vp(1000.0), PAGE, false),
        PageFit::Paginated
    );
    // … but at 150% the page needs ~1239 px, so 1000 no longer fits.
    let zoomed = Viewport::new(1000.0).with_zoom(1.5);
    assert_eq!(resolve_page_fit(zoomed, PAGE, true), PageFit::Reflow);
}

#[test]
fn unmeasured_viewport_keeps_the_current_posture() {
    assert_eq!(resolve_page_fit(vp(0.0), PAGE, true), PageFit::Paginated);
    assert_eq!(resolve_page_fit(vp(0.0), PAGE, false), PageFit::Reflow);
}
