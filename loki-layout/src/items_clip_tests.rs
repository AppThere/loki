// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the fragment-clip device-pixel floor in [`super`].

use super::clip_bottom_device_px;

/// The clip's bottom edge must be **floored** to a device pixel, never
/// rounded. Rounding up admits one more physical row, and that row is
/// exactly the top of the next line — the leak the layout used to guard
/// against by flooring in points, at two device pixels' cost per split.
#[test]
fn the_clip_bottom_floors_rather_than_rounds() {
    // .6 rounds *up* and .4 rounds down, so a rounding implementation
    // passes one of these and fails the other; only flooring passes both.
    assert!((clip_bottom_device_px(22.6, 0.0, 1.0) - 22.0).abs() < f32::EPSILON);
    assert!((clip_bottom_device_px(22.4, 0.0, 1.0) - 22.0).abs() < f32::EPSILON);

    // The floor is in *device* space: at 2× the same fractional point edge
    // keeps a half-point of precision it would lose if the layout floored.
    assert!((clip_bottom_device_px(22.6, 0.0, 2.0) - 45.0).abs() < f32::EPSILON);

    // The offset is inside the scaling, not added after it.
    assert!((clip_bottom_device_px(10.0, 2.5, 2.0) - 25.0).abs() < f32::EPSILON);
}

/// What the floor can cost is what decoration placement has to reserve.
#[test]
fn the_floor_never_shaves_more_than_the_documented_slack() {
    for scale in [1.0_f32, 1.5, 2.0, 3.0] {
        for tenth in 0..10 {
            let y = 20.0 + tenth as f32 / 10.0;
            let shaved = (y * scale - clip_bottom_device_px(y, 0.0, scale)) / scale;
            assert!(
                shaved < super::FRAGMENT_CLIP_FLOOR_SLACK_PT,
                "at scale {scale} the floor shaved {shaved}pt, over the slack \
                 `emit_spelling_squiggles` reserves",
            );
        }
    }
}
