// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::Viewport;

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
fn unmeasured_viewport_pins_left() {
    let vp = Viewport::default();
    assert_eq!(vp.inner_width_px, 0.0);
    assert_eq!(vp.centred_origin_x(816.0), 0.0);
}
