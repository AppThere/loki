// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The saturation/value square's arithmetic, separated from its rendering
//! (Spec 08 T5.2).
//!
//! # Pure, because the interesting failures are all arithmetic
//!
//! A picker that is one pixel out is invisible; a picker whose vertical axis is
//! inverted, or that clamps a drag at the wrong edge, is unusable — and both are
//! ordinary functions of two numbers. Keeping them here means they can be tested
//! without a pointer, and the component keeps only the parts that genuinely need
//! one.
//!
//! # Value runs **up**, saturation runs right
//!
//! Every colour picker in general use puts full value at the top and zero at the
//! bottom, so screen-y and value run in opposite directions. That inversion is
//! the single most likely thing to get wrong here, it looks plausible either way
//! in a screenshot, and it is why [`sv_from_position`] and [`position_from_sv`]
//! are asserted to be inverses rather than merely tested at a few points.

/// Saturation and value, each 0–100, from a pointer position inside a square of
/// `width` × `height`.
///
/// Positions outside the square clamp to its edges rather than producing a
/// value outside the range: a drag that leaves the square should pin to the
/// nearest colour, which is what every picker does and what stops a colour
/// briefly becoming impossible while the finger is still down.
#[must_use]
pub fn sv_from_position(x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
    if width <= 0.0 || height <= 0.0 {
        return (0.0, 0.0);
    }
    let saturation = (x / width).clamp(0.0, 1.0) * 100.0;
    // Inverted: the top of the square is full value.
    let value = (1.0 - (y / height).clamp(0.0, 1.0)) * 100.0;
    (saturation, value)
}

/// The handle's position inside a square of `width` × `height`, for a given
/// saturation and value. The inverse of [`sv_from_position`].
#[must_use]
pub fn position_from_sv(saturation: f32, value: f32, width: f32, height: f32) -> (f32, f32) {
    let x = saturation.clamp(0.0, 100.0) / 100.0 * width;
    let y = (1.0 - value.clamp(0.0, 100.0) / 100.0) * height;
    (x, y)
}

/// Hue 0–360 from a pointer position on a vertical strip of `height`.
///
/// Top is 0°, matching the strip's own gradient. Sharing the top-is-zero
/// convention with the gradient is what makes the colour under the handle the
/// colour the handle selects — a strip drawn one way and read the other is
/// off by the whole spectrum at the ends and correct in the middle, which is
/// exactly the error a casual look does not catch.
#[must_use]
pub fn hue_from_position(y: f32, height: f32) -> f32 {
    if height <= 0.0 {
        return 0.0;
    }
    (y / height).clamp(0.0, 1.0) * 360.0
}

/// The hue handle's offset down a strip of `height`. The inverse of
/// [`hue_from_position`].
#[must_use]
pub fn position_from_hue(hue: f32, height: f32) -> f32 {
    hue.rem_euclid(360.0) / 360.0 * height
}

/// The CSS for the hue strip's gradient, top (0°) to bottom (360°).
///
/// Built from the same six stops the sector arithmetic uses, in the same order
/// [`hue_from_position`] reads them. Written as a function rather than a
/// constant so the direction and the stops stay next to the note explaining why
/// they must agree.
#[must_use]
pub fn hue_strip_gradient() -> String {
    "linear-gradient(to bottom, #FF0000 0%, #FFFF00 16.67%, #00FF00 33.33%, \
     #00FFFF 50%, #0000FF 66.67%, #FF00FF 83.33%, #FF0000 100%)"
        .to_string()
}

#[cfg(test)]
#[path = "area_geom_tests.rs"]
mod tests;
