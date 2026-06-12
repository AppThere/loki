// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Color conversion helpers for style resolution.

use loki_primitives::color::DocumentColor;

use crate::color::LayoutColor;

/// Convert an optional [`DocumentColor`] to a [`LayoutColor`].
///
/// - `None` → [`LayoutColor::BLACK`] (default text colour).
/// - `Rgb(c)` → linear sRGB via [`LayoutColor::from`].
/// - `Transparent` → [`LayoutColor::TRANSPARENT`].
/// - `Cmyk`, `Theme`, and any future variants → [`LayoutColor::BLACK`]
///   (no ICC transform or theme resolver is available at layout time).
pub fn resolve_color(color: Option<&DocumentColor>) -> LayoutColor {
    match color {
        None => LayoutColor::BLACK,
        Some(DocumentColor::Transparent) => LayoutColor::TRANSPARENT,
        Some(DocumentColor::Rgb(rgb)) => LayoutColor::from(*rgb),
        Some(_) => LayoutColor::BLACK,
    }
}

/// Convert a [`HighlightColor`] palette entry to a [`LayoutColor`].
///
/// Returns `None` for [`HighlightColor::None`] (explicit highlight removal).
pub(crate) fn map_highlight_color(
    hc: Option<loki_doc_model::style::props::char_props::HighlightColor>,
) -> Option<LayoutColor> {
    use loki_doc_model::style::props::char_props::HighlightColor::*;
    match hc? {
        Yellow => Some(LayoutColor::new(1.000, 1.000, 0.000, 1.0)),
        Green => Some(LayoutColor::new(0.000, 1.000, 0.000, 1.0)),
        Cyan => Some(LayoutColor::new(0.000, 1.000, 1.000, 1.0)),
        Magenta => Some(LayoutColor::new(1.000, 0.000, 1.000, 1.0)),
        Blue => Some(LayoutColor::new(0.000, 0.000, 1.000, 1.0)),
        Red => Some(LayoutColor::new(1.000, 0.000, 0.000, 1.0)),
        DarkBlue => Some(LayoutColor::new(0.000, 0.000, 0.502, 1.0)),
        DarkCyan => Some(LayoutColor::new(0.000, 0.502, 0.502, 1.0)),
        DarkGreen => Some(LayoutColor::new(0.000, 0.502, 0.000, 1.0)),
        DarkMagenta => Some(LayoutColor::new(0.502, 0.000, 0.502, 1.0)),
        DarkRed => Some(LayoutColor::new(0.502, 0.000, 0.000, 1.0)),
        DarkYellow => Some(LayoutColor::new(0.502, 0.502, 0.000, 1.0)),
        DarkGray => Some(LayoutColor::new(0.502, 0.502, 0.502, 1.0)),
        LightGray => Some(LayoutColor::new(0.753, 0.753, 0.753, 1.0)),
        Black => Some(LayoutColor::BLACK),
        White => Some(LayoutColor::WHITE),
        None => Option::None,
        _ => Option::None,
    }
}
