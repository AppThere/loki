// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::{Viewport, DEFAULT_DPI};
use crate::responsive::Breakpoint;

#[test]
fn page_centres_within_a_wider_viewport() {
    // 1000-px viewport, 816-px page → (1000 − 816)/2 = 92.
    let vp = Viewport::new(1000.0);
    assert!((vp.centred_origin_x(816.0) - 92.0).abs() < 1e-4);
}

#[test]
fn reflow_tile_spanning_the_viewport_has_zero_offset() {
    // Reflow tiles span the full measured width, so the offset is 0 regardless
    // of the value — the bug class (a 1280 default vs. a 1000 real width giving
    // a spurious 140-px offset) cannot recur.
    let vp = Viewport::new(1000.0);
    assert_eq!(vp.centred_origin_x(1000.0), 0.0);
    let vp = Viewport::new(1373.0);
    assert_eq!(vp.centred_origin_x(1373.0), 0.0);
}

#[test]
fn content_wider_than_viewport_pins_left() {
    let vp = Viewport::new(600.0);
    assert_eq!(vp.centred_origin_x(816.0), 0.0);
}

#[test]
fn unmeasured_viewport_pins_left_and_is_compact() {
    let vp = Viewport::default();
    assert_eq!(vp.inner_width_px, 0.0);
    assert_eq!(vp.centred_origin_x(816.0), 0.0);
    // Mobile-first: an unmeasured viewport classifies Compact.
    assert_eq!(vp.breakpoint(), Breakpoint::Compact);
}

#[test]
fn new_defaults_zoom_and_dpi() {
    let vp = Viewport::new(800.0);
    assert_eq!(vp.zoom, 1.0);
    assert_eq!(vp.dpi, DEFAULT_DPI);
}

#[test]
fn zoom_and_dpi_setters_do_not_disturb_width_or_breakpoint() {
    let vp = Viewport::new(1280.0).with_zoom(1.5).with_dpi(144.0);
    assert_eq!(vp.zoom, 1.5);
    assert_eq!(vp.dpi, 144.0);
    assert_eq!(vp.inner_width_px, 1280.0);
    // Breakpoint depends on width only — zoom must not change the class.
    assert_eq!(vp.breakpoint(), Breakpoint::Expanded);
}

#[test]
fn breakpoint_delegates_to_width_classification() {
    assert_eq!(Viewport::new(375.0).breakpoint(), Breakpoint::Compact);
    assert_eq!(Viewport::new(800.0).breakpoint(), Breakpoint::Medium);
    assert_eq!(Viewport::new(1440.0).breakpoint(), Breakpoint::Expanded);
}
