// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the smooth-scroll easing and stepper.

use super::{animation_step, ease_out_cubic, MotionPreference};

#[test]
fn easing_spans_zero_to_one() {
    assert_eq!(ease_out_cubic(0.0), 0.0);
    assert_eq!(ease_out_cubic(1.0), 1.0);
}

#[test]
fn easing_is_clamped_outside_the_unit_interval() {
    // A tick can arrive late enough that t > 1; it must not overshoot past the
    // target, which would look like a bounce.
    assert_eq!(ease_out_cubic(1.7), 1.0);
    assert_eq!(ease_out_cubic(-0.4), 0.0);
}

#[test]
fn easing_is_monotonic_and_front_loaded() {
    let mut prev = 0.0;
    for i in 0..=10 {
        let v = ease_out_cubic(i as f32 / 10.0);
        assert!(v >= prev, "easing must not go backwards");
        prev = v;
    }
    // Ease-*out*: more than half the distance is covered in the first half of
    // the time. This is what makes the scroll feel like a response.
    assert!(ease_out_cubic(0.5) > 0.5);
}

#[test]
fn step_at_zero_is_the_start_position() {
    let (pos, done) = animation_step(100.0, 500.0, 0.0, 180.0);
    assert_eq!(pos, 100.0);
    assert!(!done);
}

#[test]
fn step_snaps_exactly_to_the_target_when_finished() {
    // Not "close to 500" — exactly 500. A residue leaves reveal_offset asking
    // for the same scroll on every subsequent caret move.
    let (pos, done) = animation_step(100.0, 500.0, 180.0, 180.0);
    assert_eq!(pos, 500.0);
    assert!(done);
}

#[test]
fn a_late_tick_finishes_rather_than_overshooting() {
    let (pos, done) = animation_step(100.0, 500.0, 10_000.0, 180.0);
    assert_eq!(pos, 500.0);
    assert!(done);
}

#[test]
fn zero_duration_completes_immediately() {
    let (pos, done) = animation_step(0.0, 42.0, 0.0, 0.0);
    assert_eq!(pos, 42.0);
    assert!(done);
}

#[test]
fn upward_scrolls_interpolate_the_same_way() {
    // Guards against an implementation that assumes to > from.
    let (pos, done) = animation_step(500.0, 100.0, 90.0, 180.0);
    assert!(!done);
    assert!(pos < 500.0 && pos > 100.0, "got {pos}");
}

#[test]
fn reduced_motion_does_not_animate() {
    assert!(MotionPreference::Full.animates());
    assert!(!MotionPreference::Reduced.animates());
    assert_eq!(MotionPreference::default(), MotionPreference::Full);
}
