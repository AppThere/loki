// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the rotated-group content CTM.

use super::rotated_group_ctm;

/// Apply a PDF `[a b c d e f]` matrix to a point.
fn apply(m: [f32; 6], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// The per-leaf y-flip the PDF renderer bakes into every leaf.
fn flip(page_h: f32, x: f32, y: f32) -> (f32, f32) {
    (x, page_h - y)
}

#[test]
fn zero_degrees_is_a_plain_offset_and_flip() {
    // With no rotation the CTM must reproduce the old "render children at the
    // group origin" behaviour: translate by the absolute origin in x, and by the
    // negated origin in y (the flip folds the two y-reflections into a sign).
    // Absolute origin = area offset (5,7) + group origin (10,20) = (15,27).
    let m = rotated_group_ctm(15.0, 27.0, 0.0, 100.0, 40.0, 800.0);
    assert_eq!(m, [1.0, 0.0, 0.0, 1.0, 15.0, -27.0]);
}

#[test]
fn ninety_degrees_matches_hand_computed_matrix() {
    // Derived in the module docs: origin (0,0), 100×40 group, 200 pt page.
    let m = rotated_group_ctm(0.0, 0.0, 90.0, 100.0, 40.0, 200.0);
    for (got, want) in m.iter().zip([0.0, -1.0, 1.0, 0.0, -160.0, 200.0]) {
        assert!((got - want).abs() < 1e-3, "matrix {m:?}");
    }
}

#[test]
fn device_point_equals_screen_rotation_flipped() {
    // The contract: rendering a child leaf (which emits `flip(p_local)`) under
    // the CTM must land at `flip(screen_rotation(p_local))`. Check the four
    // corners of a 90°-rotated 100×40 group against the on-screen transform.
    let page_h = 200.0;
    let m = rotated_group_ctm(0.0, 0.0, 90.0, 100.0, 40.0, page_h);
    // On-screen M for this case (module docs): (x, y) → (40 − y, x).
    let screen = |x: f32, y: f32| (40.0 - y, x);
    for (lx, ly) in [(0.0, 0.0), (100.0, 0.0), (100.0, 40.0), (0.0, 40.0)] {
        let (qx, qy) = flip(page_h, lx, ly);
        let device = apply(m, qx, qy);
        let (sx, sy) = screen(lx, ly);
        let expected = flip(page_h, sx, sy);
        assert!(
            (device.0 - expected.0).abs() < 1e-3 && (device.1 - expected.1).abs() < 1e-3,
            "local ({lx},{ly}): device {device:?} != expected {expected:?}"
        );
    }
}

#[test]
fn origin_and_area_offset_shift_the_group() {
    // The caller folds margins + group origin into the absolute position; at
    // θ=0 the x-translation is that position's x and the y is its negation.
    let m = rotated_group_ctm(72.0 + 30.0, 72.0 + 40.0, 0.0, 50.0, 50.0, 500.0);
    assert_eq!(m[4], 72.0 + 30.0);
    assert_eq!(m[5], -(72.0 + 40.0));
}
