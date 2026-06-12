// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Border edge mapper: [`DocxBorderEdge`] → [`Border`].

use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use crate::docx::model::paragraph::DocxBorderEdge;
use crate::xml_util::hex_color;

/// Maps a `DocxBorderEdge` to a doc-model [`Border`].
///
/// `"nil"` and `"none"` produce [`BorderStyle::None`]. `@w:sz` is in eighths
/// of a point (ECMA-376 §17.3.4); `@w:space` is in points (not twips).
pub(crate) fn map_border_edge(edge: &DocxBorderEdge) -> Border {
    let style = match edge.val.as_str() {
        "nil" | "none" => BorderStyle::None,
        "double" => BorderStyle::Double,
        "dashed" | "dashSmallGap" | "dashDot" | "dashDotDot" | "dotDash" | "dotDotDash"
        | "dashDotStroked" => BorderStyle::Dashed,
        "dotted" | "dottedHeavy" => BorderStyle::Dotted,
        "wave" | "wavyHeavy" | "wavyDouble" => BorderStyle::Wave,
        _ => BorderStyle::Solid,
    };
    Border {
        style,
        width: Points::new(f64::from(edge.sz.unwrap_or(8)) / 8.0),
        color: edge
            .color
            .as_deref()
            .and_then(hex_color)
            .map(DocumentColor::Rgb),
        spacing: edge.space.map(|s| Points::new(f64::from(s))),
    }
}
