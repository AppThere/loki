// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The SV square's and hue strip's arithmetic.

use super::{
    hue_from_position, hue_strip_gradient, position_from_hue, position_from_sv, sv_from_position,
};

const W: f32 = 200.0;
const H: f32 = 160.0;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// **The inversion, stated as directly as it can be.** Full value is at the
/// *top*; a picker that reads screen-y as value straight through is upside down,
/// and it looks plausible either way in a screenshot.
#[test]
fn the_top_of_the_square_is_full_value() {
    let (_, top) = sv_from_position(0.0, 0.0, W, H);
    let (_, bottom) = sv_from_position(0.0, H, W, H);
    assert!(close(top, 100.0), "top was {top}");
    assert!(close(bottom, 0.0), "bottom was {bottom}");
    assert!(top > bottom, "value must increase upward");
}

/// Saturation runs left to right, which is the axis that is *not* inverted —
/// asserted so a fix to the other axis cannot quietly flip this one too.
#[test]
fn saturation_increases_to_the_right() {
    let (left, _) = sv_from_position(0.0, 0.0, W, H);
    let (right, _) = sv_from_position(W, 0.0, W, H);
    assert!(close(left, 0.0));
    assert!(close(right, 100.0));
}

/// **Position and value are inverses, everywhere.** Checked as a round trip
/// rather than at a few points, so it keeps holding if the geometry changes.
#[test]
fn a_position_round_trips_through_saturation_and_value() {
    for x in [0.0, 1.0, 50.0, 123.4, W] {
        for y in [0.0, 1.0, 80.0, 159.0, H] {
            let (s, v) = sv_from_position(x, y, W, H);
            let (bx, by) = position_from_sv(s, v, W, H);
            assert!(close(bx, x), "x {x} -> s {s} -> {bx}");
            assert!(close(by, y), "y {y} -> v {v} -> {by}");
        }
    }
}

/// **A drag that leaves the square pins to its edge.** Not clamping would let a
/// finger dragged past the corner produce a saturation above 100, which is a
/// colour that does not exist — briefly, while the finger is still down.
#[test]
fn a_drag_outside_the_square_clamps_to_it() {
    let (s, v) = sv_from_position(-40.0, -40.0, W, H);
    assert!(close(s, 0.0) && close(v, 100.0), "top-left, got {s}/{v}");
    let (s, v) = sv_from_position(W + 40.0, H + 40.0, W, H);
    assert!(
        close(s, 100.0) && close(v, 0.0),
        "bottom-right, got {s}/{v}"
    );
}

/// A square with no size yields no colour rather than a division by zero — the
/// state before the control has been measured.
#[test]
fn an_unmeasured_square_yields_no_colour() {
    assert_eq!(sv_from_position(10.0, 10.0, 0.0, 0.0), (0.0, 0.0));
    assert_eq!(hue_from_position(10.0, 0.0), 0.0);
}

/// The strip runs 0° at the top to 360° at the bottom, and round-trips.
#[test]
fn the_hue_strip_runs_from_zero_at_the_top() {
    assert!(close(hue_from_position(0.0, H), 0.0));
    assert!(close(hue_from_position(H, H), 360.0));
    assert!(close(hue_from_position(H / 2.0, H), 180.0));
    for hue in [0.0, 45.0, 120.0, 240.0, 359.0] {
        let back = hue_from_position(position_from_hue(hue, H), H);
        assert!(close(back, hue), "hue {hue} round-tripped to {back}");
    }
}

/// **The gradient and the reader must agree, or the strip is a lie.** The
/// colour painted at a position has to be the colour selecting there produces;
/// a strip drawn one way and read the other is correct in the middle and wrong
/// at both ends, which is what a casual look does not catch.
///
/// Checked structurally: the stops are in ascending hue order top-to-bottom, in
/// the same direction `hue_from_position` reads.
#[test]
fn the_strip_gradient_runs_the_same_way_it_is_read() {
    let css = hue_strip_gradient();
    assert!(css.contains("to bottom"), "{css}");
    let order = [
        "#FF0000 0%",
        "#FFFF00",
        "#00FF00",
        "#00FFFF",
        "#0000FF",
        "#FF00FF",
    ];
    let mut last = 0;
    for stop in order {
        let at = css
            .find(stop)
            .unwrap_or_else(|| panic!("missing {stop} in {css}"));
        assert!(at >= last, "stop {stop} out of order in {css}");
        last = at;
    }
    assert!(
        css.trim_end().ends_with("100%)"),
        "the turn must close on red: {css}"
    );
}
