// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What a wheel gesture does to the zoom.

use super::wheel_zoom_percent;
use appthere_ui::{ZOOM_MAX_PERCENT, ZOOM_MIN_PERCENT};
use dioxus::html::geometry::WheelDelta;

/// One notch of a classic wheel, pushed away from the reader.
fn up(lines: f64) -> WheelDelta {
    WheelDelta::lines(0.0, lines, 0.0)
}

/// A trackpad gesture, in pixels.
fn px(y: f64) -> WheelDelta {
    WheelDelta::pixels(0.0, y, 0.0)
}

/// Positive delta zooms **in** — winit's sign, which is the wheel pushed away
/// from the reader.
#[test]
fn pushing_the_wheel_away_zooms_in() {
    assert!(wheel_zoom_percent(100, up(1.0)) > 100);
}

/// **And negative zooms out.** Without this the direction test above passes on a
/// function that ignores the sign and always increases.
#[test]
fn pulling_the_wheel_back_zooms_out() {
    assert!(wheel_zoom_percent(100, up(-1.0)) < 100);
}

/// A gesture with no vertical movement changes nothing — a horizontal trackpad
/// swipe with Ctrl held is not a zoom.
#[test]
fn a_zero_delta_does_nothing() {
    assert_eq!(wheel_zoom_percent(137, up(0.0)), 137);
    assert_eq!(
        wheel_zoom_percent(137, WheelDelta::pixels(400.0, 0.0, 0.0)),
        137
    );
}

/// **The step is multiplicative**, so it is the same *visual* size everywhere.
/// Measured as the point-change, which must be far larger at 400% than at 50% —
/// a fixed ±N points would make these equal, which is the design this rejects.
#[test]
fn the_step_scales_with_the_zoom() {
    let low = wheel_zoom_percent(50, up(1.0)) - 50;
    let high = wheel_zoom_percent(400, up(1.0)) - 400;
    assert!(
        high > low * 4,
        "step at 400% ({high}) should dwarf the step at 50% ({low})"
    );
}

/// **The floor is not doing all the work.** Away from the range's bottom the
/// multiplicative result already differs from `current`, so the minimum-one-
/// percent branch is *false* here — which is what makes it a precondition rather
/// than a description of every call.
#[test]
fn the_one_percent_floor_is_not_always_taken() {
    // 1.1 × 100 = 110, nine points clear of the floor's 101.
    assert_eq!(wheel_zoom_percent(100, up(1.0)), 110);
}

/// **And it does fire where it is needed.** At the bottom of the range a small
/// trackpad delta rounds back to where it started (1% of 20 is 0.2), so without
/// the floor the gesture would be dead at 20–25% while working fine at 100% —
/// a control that fails only where the reader is most likely to be nudging it.
#[test]
fn a_small_gesture_still_moves_at_the_bottom_of_the_range() {
    let after = wheel_zoom_percent(ZOOM_MIN_PERCENT, px(1.0));
    assert_eq!(after, ZOOM_MIN_PERCENT + 1, "a 1px gesture must still move");
}

/// The same downward, off the floor so the clamp is not what is being measured.
#[test]
fn a_small_gesture_still_moves_downward() {
    assert_eq!(wheel_zoom_percent(ZOOM_MIN_PERCENT + 1, px(-1.0)), 21 - 1);
}

/// The range holds at the top, however hard the gesture is thrown.
#[test]
fn it_clamps_at_the_ceiling() {
    assert_eq!(wheel_zoom_percent(590, up(50.0)), ZOOM_MAX_PERCENT);
    assert_eq!(
        wheel_zoom_percent(ZOOM_MAX_PERCENT, up(1.0)),
        ZOOM_MAX_PERCENT
    );
}

/// **And at the floor** — including the case the one-percent step would
/// otherwise walk past: at 20% a downward nudge must stay at 20, not reach 19.
#[test]
fn it_clamps_at_the_floor() {
    assert_eq!(wheel_zoom_percent(30, up(-50.0)), ZOOM_MIN_PERCENT);
    assert_eq!(
        wheel_zoom_percent(ZOOM_MIN_PERCENT, px(-1.0)),
        ZOOM_MIN_PERCENT
    );
}

/// Pixels and lines are different units and are treated as such: 50 px is one
/// line by this module's scale, so the two must land on the same zoom.
#[test]
fn pixels_convert_to_lines_at_the_stated_scale() {
    assert_eq!(
        wheel_zoom_percent(100, px(50.0)),
        wheel_zoom_percent(100, up(1.0))
    );
}

/// **A pixel delta is not read as a line count.** If the unit were dropped, 50 px
/// would be fifty lines and one trackpad event would cross the whole range.
#[test]
fn a_pixel_delta_is_not_fifty_lines() {
    assert!(wheel_zoom_percent(100, px(50.0)) < wheel_zoom_percent(100, up(50.0)));
}

/// A platform reporting pages moves conservatively rather than not at all.
#[test]
fn pages_are_not_ignored() {
    assert!(wheel_zoom_percent(100, WheelDelta::pages(0.0, 1.0, 0.0)) > 100);
}

/// A non-finite delta cannot reach the zoom. `powf` on a NaN yields NaN, and
/// `NaN as i64` is 0 — which would silently clamp the zoom to its floor.
#[test]
fn a_non_finite_delta_is_refused() {
    assert_eq!(wheel_zoom_percent(137, px(f64::NAN)), 137);
    assert_eq!(wheel_zoom_percent(137, px(f64::INFINITY)), 137);
    assert_eq!(wheel_zoom_percent(137, px(f64::NEG_INFINITY)), 137);
}

/// Every zoom the gesture can produce is inside the range, from every starting
/// point in it — the property the individual clamp tests sample.
#[test]
fn no_gesture_leaves_the_range() {
    for current in ZOOM_MIN_PERCENT..=ZOOM_MAX_PERCENT {
        for delta in [-100.0, -3.0, -0.02, 0.02, 3.0, 100.0] {
            let after = wheel_zoom_percent(current, up(delta));
            assert!(
                (ZOOM_MIN_PERCENT..=ZOOM_MAX_PERCENT).contains(&after),
                "{current}% + {delta} lines left the range at {after}%"
            );
        }
    }
}
