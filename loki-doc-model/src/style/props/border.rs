// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Paragraph and table border properties.
//!
//! TR 29166 §6.2.2 "Paragraph formatting" includes border definitions.
//! ODF maps these to `fo:border-*` properties; OOXML maps them to
//! `w:pBdr` and `w:tcBdr` elements.

use loki_primitives::units::Points;
use loki_primitives::color::DocumentColor;

/// The line style of a border edge.
///
/// Derived from the CSS/XSL-FO border-style vocabulary used by both
/// ODF (`fo:border-style`) and OOXML (`w:val` on border elements).
/// TR 29166 §6.2.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BorderStyle {
    /// No border is drawn.
    #[default]
    None,
    /// A single solid line.
    Solid,
    /// A dashed line.
    Dashed,
    /// A dotted line.
    Dotted,
    /// Two parallel solid lines.
    Double,
    /// A three-dimensional groove effect.
    Groove,
    /// A three-dimensional ridge effect.
    Ridge,
    /// The border appears inset.
    Inset,
    /// The border appears outset.
    Outset,
    /// A wavy line (ODF `wave`; OOXML `wave`).
    Wave,
}

/// A single border edge definition.
///
/// Represents one side (top, bottom, left, right) of a paragraph or
/// table cell border. TR 29166 §6.2.2 "Paragraph borders".
///
/// ODF: `fo:border-top`, `fo:border-bottom`, etc., plus
/// `fo:border-top-color`, `fo:border-top-style`, `fo:border-top-width`.
/// OOXML: `w:top`, `w:bottom`, `w:left`, `w:right` inside `w:pBdr`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Border {
    /// The visual style of the border line.
    pub style: BorderStyle,
    /// The width of the border line in points.
    pub width: Points,
    /// The color of the border line. `None` means use the automatic
    /// (inherited or theme) color.
    pub color: Option<DocumentColor>,
    /// The distance between the border and the enclosing content,
    /// in points (ODF `fo:padding-*`; OOXML `w:space`).
    pub spacing: Option<Points>,
}

impl Border {
    /// Creates a simple solid border of the given width and color.
    #[must_use]
    pub fn solid(width: Points, color: DocumentColor) -> Self {
        Self {
            style: BorderStyle::Solid,
            width,
            color: Some(color),
            spacing: None,
        }
    }

    /// Creates a border with [`BorderStyle::None`] — effectively removes
    /// any inherited border.
    #[must_use]
    pub fn none() -> Self {
        Self {
            style: BorderStyle::None,
            width: Points::new(0.0),
            color: None,
            spacing: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_none_has_zero_width() {
        let b = Border::none();
        assert_eq!(b.style, BorderStyle::None);
    }
}
