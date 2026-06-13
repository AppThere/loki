// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Preset (built-in) shape geometries.
//!
//! A preset is parameterised only by its enclosing shape frame, so the renderer
//! resolves the actual outline from the frame size. This mirrors the preset
//! geometry sets in OOXML DrawingML (`prstGeom`) and ODF draw shapes. Custom
//! outlines use [`crate::Path`] instead.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A built-in shape outline, drawn to fill the shape's frame.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PresetShape {
    /// Axis-aligned rectangle filling the frame.
    Rectangle,
    /// Rectangle with rounded corners.
    RoundedRectangle {
        /// Corner radius in points.
        corner_radius: f64,
    },
    /// Ellipse inscribed in the frame.
    Ellipse,
    /// A straight line along the frame diagonal (top-left → bottom-right).
    Line,
    /// Isosceles triangle pointing up.
    Triangle,
    /// Right triangle with the right angle at the bottom-left.
    RightTriangle,
    /// Diamond (rhombus) inscribed in the frame.
    Diamond,
    /// Regular-ish pentagon inscribed in the frame.
    Pentagon,
    /// Hexagon inscribed in the frame.
    Hexagon,
}
