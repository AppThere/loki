// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The custom section's colour models, their field labels, and the parse from
//! typed fields to RGB.
//!
//! Split out of `custom.rs` for the 300-line ceiling. It is a clean seam: this
//! is the *typed* vocabulary, where the rest of `custom.rs` is the rendering and
//! the square. That distinction is load-bearing rather than tidy — Spec 08 T5.3
//! found the Apply button calling [`resolve`] when it meant the displayed
//! colour, and the two are only equal when the reader typed.

use super::convert::{cmyk_to_rgb, hsl_to_rgb, hsv_to_rgb, parse_hex};
use super::custom_source::FieldMode;

/// The supported colour-entry models. Labels are technical abbreviations.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Mode {
    Hex,
    Rgb,
    Hsl,
    Hsv,
    Cmyk,
}

pub(super) const MODES: &[(Mode, &str)] = &[
    (Mode::Hex, "Hex"),
    (Mode::Rgb, "RGB"),
    (Mode::Hsl, "HSL"),
    (Mode::Hsv, "HSV"),
    (Mode::Cmyk, "CMYK"),
];

/// `(field labels, default values)` for a mode. At most four fields are used;
/// unused entries are empty.
pub(super) fn fields_for(mode: Mode) -> &'static [&'static str] {
    match mode {
        Mode::Hex => &["#"],
        Mode::Rgb => &["R", "G", "B"],
        Mode::Hsl => &["H", "S", "L"],
        Mode::Hsv => &["H", "S", "V"],
        Mode::Cmyk => &["C", "M", "Y", "K"],
    }
}

/// This module's `Mode` as the display module's [`FieldMode`].
///
/// Two enums with one mapping rather than one shared enum, because `Mode` is
/// this module's private business and `FieldMode` is about what the controls
/// render. The mapping is exhaustive, so adding a model here is a compile error
/// there rather than a mode that silently shows nothing.
pub(super) fn field_mode(mode: Mode) -> FieldMode {
    match mode {
        Mode::Hex => FieldMode::Hex,
        Mode::Rgb => FieldMode::Rgb,
        Mode::Hsl => FieldMode::Hsl,
        Mode::Hsv => FieldMode::Hsv,
        Mode::Cmyk => FieldMode::Cmyk,
    }
}

/// Parses the current field values under `mode` into RGB bytes.
pub(super) fn resolve(mode: Mode, f: &[String; 4]) -> Option<(u8, u8, u8)> {
    let num = |s: &String| s.trim().parse::<f32>().ok().filter(|v| v.is_finite());
    match mode {
        Mode::Hex => parse_hex(&f[0]),
        Mode::Rgb => {
            let (r, g, b) = (num(&f[0])?, num(&f[1])?, num(&f[2])?);
            let in_range = |v: f32| (0.0..=255.0).contains(&v);
            (in_range(r) && in_range(g) && in_range(b))
                .then(|| (r.round() as u8, g.round() as u8, b.round() as u8))
        }
        Mode::Hsl => Some(hsl_to_rgb(num(&f[0])?, num(&f[1])?, num(&f[2])?)),
        Mode::Hsv => Some(hsv_to_rgb(num(&f[0])?, num(&f[1])?, num(&f[2])?)),
        Mode::Cmyk => Some(cmyk_to_rgb(
            num(&f[0])?,
            num(&f[1])?,
            num(&f[2])?,
            num(&f[3])?,
        )),
    }
}

#[cfg(test)]
#[path = "custom_mode_tests.rs"]
mod tests;
