// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Internal conversion helpers shared by the para and char mappers.

use loki_doc_model::style::props::char_props::{HighlightColor, UnderlineStyle};
use loki_doc_model::style::props::para_props::LineHeight;
use loki_primitives::units::Points;

/// Converts a twips integer to [`Points`] (1 pt = 20 twips).
pub(super) fn twips_to_pt(twips: i32) -> Points {
    Points::new(f64::from(twips) / 20.0)
}

/// Maps a `w:jc` value string to a [`loki_doc_model::style::props::para_props::ParagraphAlignment`].
pub(super) fn map_jc(jc: &str) -> loki_doc_model::style::props::para_props::ParagraphAlignment {
    use loki_doc_model::style::props::para_props::ParagraphAlignment;
    match jc {
        "both" | "distribute" => ParagraphAlignment::Justify,
        "center" => ParagraphAlignment::Center,
        "right" | "end" => ParagraphAlignment::Right,
        _ => ParagraphAlignment::Left,
    }
}

/// Maps `w:line` + `w:lineRule` to [`LineHeight`].
///
/// - `lineRule="exact"` → [`LineHeight::Exact`] (pt)
/// - `lineRule="atLeast"` → [`LineHeight::AtLeast`] (pt)
/// - `lineRule="auto"` or absent → [`LineHeight::Multiple`] (line/240.0)
pub(super) fn map_line_height(line: i32, line_rule: Option<&str>) -> LineHeight {
    match line_rule {
        Some("exact") => LineHeight::Exact(twips_to_pt(line)),
        Some("atLeast") => LineHeight::AtLeast(twips_to_pt(line)),
        #[allow(clippy::cast_precision_loss)]
        // Precision loss acceptable: values represent document measurements
        _ => LineHeight::Multiple(line as f32 / 240.0),
    }
}

/// Maps a `w:u @w:val` string to [`UnderlineStyle`].
///
/// Returns `None` for `"none"` (explicit removal of underline).
pub(super) fn map_underline(val: &str) -> Option<UnderlineStyle> {
    match val {
        "none" => None,
        "double" => Some(UnderlineStyle::Double),
        "thick" | "thickDash" | "thickDotDash" | "thickDotDotDash" | "thickDotted" => {
            Some(UnderlineStyle::Thick)
        }
        "dotted" | "dottedHeavy" => Some(UnderlineStyle::Dotted),
        "dash" | "dashedHeavy" | "dashLong" | "dashLongHeavy" => Some(UnderlineStyle::Dash),
        "wave" | "wavyHeavy" | "wavyDouble" => Some(UnderlineStyle::Wave),
        _ => Some(UnderlineStyle::Single),
    }
}

/// Maps a `w:highlight @w:val` string to [`HighlightColor`].
pub(super) fn map_highlight(val: &str) -> HighlightColor {
    match val {
        "black" => HighlightColor::Black,
        "blue" => HighlightColor::Blue,
        "cyan" => HighlightColor::Cyan,
        "darkBlue" => HighlightColor::DarkBlue,
        "darkCyan" => HighlightColor::DarkCyan,
        "darkGray" => HighlightColor::DarkGray,
        "darkGreen" => HighlightColor::DarkGreen,
        "darkMagenta" => HighlightColor::DarkMagenta,
        "darkRed" => HighlightColor::DarkRed,
        "darkYellow" => HighlightColor::DarkYellow,
        "green" => HighlightColor::Green,
        "lightGray" => HighlightColor::LightGray,
        "magenta" => HighlightColor::Magenta,
        "red" => HighlightColor::Red,
        "white" => HighlightColor::White,
        "yellow" => HighlightColor::Yellow,
        _ => HighlightColor::None,
    }
}
