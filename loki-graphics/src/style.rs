// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Fill and stroke styling for shapes.
//!
//! Colors are [`DocumentColor`]s, so theme colors (and their tints) survive
//! through to presentation themes — a presentation accent fill stays semantic
//! rather than being flattened to RGB at model time.

use loki_primitives::color::DocumentColor;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// How a shape's interior is painted.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Fill {
    /// No fill (transparent interior).
    #[default]
    None,
    /// A single solid color.
    Solid(DocumentColor),
    /// A linear gradient.
    LinearGradient(LinearGradient),
}

impl Fill {
    /// Convenience constructor for a solid fill.
    pub fn solid(color: DocumentColor) -> Self {
        Fill::Solid(color)
    }
}

/// A linear gradient fill.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LinearGradient {
    /// Color stops, ordered by offset.
    pub stops: Vec<GradientStop>,
    /// Gradient direction in degrees (0 = left→right, 90 = top→bottom).
    pub angle_deg: f64,
}

/// A single gradient color stop.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GradientStop {
    /// Position along the gradient, `0.0..=1.0`.
    pub offset: f64,
    /// Stop color.
    pub color: DocumentColor,
}

/// A shape outline stroke.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Stroke {
    /// Stroke color.
    pub color: DocumentColor,
    /// Stroke width in points.
    pub width_pt: f64,
    /// Dash pattern.
    pub dash: LineDash,
    /// How stroke ends are drawn.
    pub cap: LineCap,
    /// How stroke corners are joined.
    pub join: LineJoin,
}

impl Stroke {
    /// A solid stroke of the given color and width, with default cap/join.
    pub fn solid(color: DocumentColor, width_pt: f64) -> Self {
        Self {
            color,
            width_pt,
            dash: LineDash::Solid,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
        }
    }
}

/// Stroke dash pattern (a small preset set; custom dashes can come later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LineDash {
    /// Continuous line.
    #[default]
    Solid,
    /// Evenly spaced dashes.
    Dash,
    /// Round/square dots.
    Dot,
    /// Alternating dash and dot.
    DashDot,
}

/// Stroke end-cap style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LineCap {
    /// Squared off at the endpoint.
    #[default]
    Butt,
    /// Rounded past the endpoint.
    Round,
    /// Squared off past the endpoint.
    Square,
}

/// Stroke corner-join style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LineJoin {
    /// Sharp mitered corner.
    #[default]
    Miter,
    /// Rounded corner.
    Round,
    /// Beveled (flattened) corner.
    Bevel,
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_primitives::color::DocumentColor;

    #[test]
    fn fill_defaults_to_none() {
        assert_eq!(Fill::default(), Fill::None);
    }

    #[test]
    fn solid_stroke_uses_default_cap_join() {
        let s = Stroke::solid(DocumentColor::from_hex("#000000").unwrap(), 1.5);
        assert_eq!(s.width_pt, 1.5);
        assert_eq!(s.cap, LineCap::Butt);
        assert_eq!(s.join, LineJoin::Miter);
        assert_eq!(s.dash, LineDash::Solid);
    }
}
