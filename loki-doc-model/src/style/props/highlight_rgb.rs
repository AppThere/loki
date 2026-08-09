// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The RGB each named highlight paints as, and the reverse lookup (Spec 08 T5.3).
//!
//! # One table, because there were three
//!
//! `HighlightColor` is a fixed enumeration — that is what `w:highlight` is — but
//! the *colours* it names were written out separately in `loki-layout`'s
//! `map_highlight_color` (as float triples) and in `loki-text`'s
//! `HIGHLIGHT_PALETTE` (as hex strings, with a comment saying they "mirror"
//! the layout ones). Offering an arbitrary highlight colour needs a third
//! reading — hex back to a variant — and adding it beside the other two is how
//! the same yellow comes to export two different ways depending on where the
//! reader clicked.
//!
//! So the palette lives here, next to the enum it describes, and the other
//! readings derive from it. L08-029: one fact, one derivation.
//!
//! # The values
//!
//! The sixteen `w:highlight` colours are the VGA/HTML-4 basic palette: full
//! channels for the bright half and `0x80` (`0.502`) for the dark half, with
//! `LightGray` at `0xC0`. They are fixed by the format, not by taste — do not
//! adjust them to match a theme.

use loki_primitives::color::RgbColor;

use super::char_props::HighlightColor;

/// Every named highlight and its `#RRGGBB`, in the palette's conventional order
/// (bright colours, then dark, then greys, then black and white).
///
/// [`HighlightColor::None`] is deliberately absent: it means *no highlight*, and
/// a colour for it would be a colour to paint.
pub const HIGHLIGHT_RGB: &[(HighlightColor, &str)] = &[
    (HighlightColor::Yellow, "#FFFF00"),
    (HighlightColor::Green, "#00FF00"),
    (HighlightColor::Cyan, "#00FFFF"),
    (HighlightColor::Magenta, "#FF00FF"),
    (HighlightColor::Blue, "#0000FF"),
    (HighlightColor::Red, "#FF0000"),
    (HighlightColor::DarkBlue, "#000080"),
    (HighlightColor::DarkCyan, "#008080"),
    (HighlightColor::DarkGreen, "#008000"),
    (HighlightColor::DarkMagenta, "#800080"),
    (HighlightColor::DarkRed, "#800000"),
    (HighlightColor::DarkYellow, "#808000"),
    (HighlightColor::DarkGray, "#808080"),
    (HighlightColor::LightGray, "#C0C0C0"),
    (HighlightColor::Black, "#000000"),
    (HighlightColor::White, "#FFFFFF"),
];

impl HighlightColor {
    /// This highlight's `#RRGGBB`, or `None` for [`HighlightColor::None`].
    #[must_use]
    pub fn to_hex(self) -> Option<&'static str> {
        HIGHLIGHT_RGB
            .iter()
            .find(|(v, _)| *v == self)
            .map(|(_, hex)| *hex)
    }

    /// This highlight's colour, or `None` for [`HighlightColor::None`].
    #[must_use]
    pub fn to_rgb(self) -> Option<RgbColor> {
        self.to_hex().and_then(parse_hex)
    }

    /// The named highlight whose colour is **exactly** `hex`, if any.
    ///
    /// # Exact, and on the colour rather than on the pick
    ///
    /// This is the whole point of the reverse lookup. A reader who types
    /// `#FFFF00` into the hex field means the same yellow as one who clicks the
    /// Yellow swatch, and if the two took different routes the same colour would
    /// export as `w:highlight` from one and `w:shd` from the other — visible in
    /// Word as two different controls for what the reader sees as one colour.
    ///
    /// Deliberately **not** nearest-match. Snapping `#FFFF01` to Yellow would
    /// silently discard a colour the reader chose; the honest result is that it
    /// is a custom colour, which the model can now carry.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        let want = normalise(hex)?;
        HIGHLIGHT_RGB
            .iter()
            .find(|(_, h)| normalise(h).is_some_and(|n| n == want))
            .map(|(v, _)| *v)
    }
}

/// `#RRGGBB` (case- and `#`-insensitive) as three bytes.
fn normalise(hex: &str) -> Option<[u8; 3]> {
    let s = hex.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// `#RRGGBB` as an [`RgbColor`]. Private: the crate's colour parsing entry
/// point is [`HighlightColor::to_rgb`], so there is one route in.
fn parse_hex(hex: &str) -> Option<RgbColor> {
    let [r, g, b] = normalise(hex)?;
    Some(RgbColor::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ))
}

#[cfg(test)]
#[path = "highlight_rgb_tests.rs"]
mod tests;
