// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Which control last spoke, and therefore which colour the section is showing
//! (Spec 08 T5.2).
//!
//! # Two controls, one colour, and a rule about who wins
//!
//! The custom section now has a saturation/value square *and* the numeric
//! fields, and both name a colour. Without a rule the two fight: the square
//! writes the fields on every drag frame, the fields re-derive the square's
//! handle, and a rounding difference between them makes the handle creep while
//! the reader holds still.
//!
//! The rule is **last edited wins**, which is what a person already assumes:
//! drag the square and the square is the colour; type in a field and the field
//! is. Nothing is written back, so nothing can round-trip and drift.
//!
//! # The handle still tracks a typed colour
//!
//! Not writing back does not mean not following. When the fields are the source,
//! the square's handle is *derived* from them on every render — which is
//! one-directional and therefore cannot drift, and is why
//! [`convert::hue_preserving_hsv`](super::convert::hue_preserving_hsv) exists:
//! a typed grey names no hue, and recomputing one from it would throw the strip
//! to red.

use super::convert::hue_preserving_hsv;

/// Which control the displayed colour comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum Source {
    /// The numeric fields — the state before anything is dragged.
    #[default]
    Fields,
    /// The saturation/value square and hue strip.
    Area,
}

/// The hue, saturation and value the square should show.
///
/// `area_hsv` is what the reader last dragged; `resolved` is what the fields
/// currently parse to. When the fields are the source the handle follows them,
/// keeping `area_hsv`'s hue for a colour that names none.
#[must_use]
pub(super) fn displayed_hsv(
    source: Source,
    area_hsv: (f32, f32, f32),
    resolved: Option<(u8, u8, u8)>,
) -> (f32, f32, f32) {
    match (source, resolved) {
        (Source::Area, _) => area_hsv,
        // Unparseable fields leave the handle where it was rather than moving it
        // to black: a half-typed hex is a transient state, and a handle that
        // jumped to the corner on every keystroke would be unusable.
        (Source::Fields, None) => area_hsv,
        (Source::Fields, Some((r, g, b))) => hue_preserving_hsv(r, g, b, area_hsv.0),
    }
}

/// The text each field should *show*, for a colour the square produced.
///
/// # Showing is not writing back
///
/// The "last edited wins" rule says the fields do not *store* what the square
/// produced. It does not say they should sit empty while a colour is plainly
/// selected — a reader who drags a colour and wants its hex to paste elsewhere
/// has nowhere to read it, and the first screen sitting showed exactly that: a
/// filled preview swatch beside a blank `#` field.
///
/// So the fields display a value derived on every render from the square. That
/// is one-directional and cannot drift, which is the property the rule exists to
/// protect. The moment the reader types, `Source::Fields` takes over and their
/// text is what shows.
#[must_use]
pub(super) fn fields_from_rgb(mode: FieldMode, (r, g, b): (u8, u8, u8)) -> [String; 4] {
    let e = String::new();
    match mode {
        FieldMode::Hex => [super::convert::rgb_to_hex(r, g, b), e.clone(), e.clone(), e],
        FieldMode::Rgb => [r.to_string(), g.to_string(), b.to_string(), e],
        FieldMode::Hsl => {
            let (h, s, l) = super::convert::rgb_to_hsl(r, g, b);
            [round(h), round(s), round(l), e]
        }
        FieldMode::Hsv => {
            let (h, s, v) = super::convert::rgb_to_hsv(r, g, b);
            [round(h), round(s), round(v), e]
        }
        // CMYK has no inverse in this crate — the forward conversion is the
        // naive complement and is documented as adequate for a preview only, so
        // deriving a display value would present a number with more authority
        // than it has. The fields stay blank in that mode, which is honest.
        FieldMode::Cmyk => [e.clone(), e.clone(), e.clone(), e],
    }
}

fn round(v: f32) -> String {
    format!("{}", v.round() as i32)
}

/// The colour models the fields can display, mirroring `custom::Mode`.
///
/// A separate enum because `Mode` is private to `custom` and this module is
/// about what the *controls* show; coupling them would make either one's
/// visibility the other's problem.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FieldMode {
    Hex,
    Rgb,
    Hsl,
    Hsv,
    Cmyk,
}

#[cfg(test)]
#[path = "custom_source_tests.rs"]
mod tests;
