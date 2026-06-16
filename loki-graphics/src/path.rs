// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Custom path geometry.
//!
//! A [`Path`] is a sequence of [`PathSegment`]s in the shape's **local**
//! coordinate space (origin at the shape frame's top-left, extent equal to the
//! frame size). Placement, rotation, and flips are applied by the shape's
//! [`crate::ShapeTransform`], mirroring the OOXML/ODF "geometry in a box, placed
//! by a transform" model.

use crate::geometry::Vec2;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A single path command.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PathSegment {
    /// Begin a new subpath at the given point.
    MoveTo(Vec2),
    /// Straight line to the given point.
    LineTo(Vec2),
    /// Quadratic Bézier with one control point.
    QuadTo {
        /// Control point.
        ctrl: Vec2,
        /// End point.
        to: Vec2,
    },
    /// Cubic Bézier with two control points.
    CubicTo {
        /// First control point.
        c1: Vec2,
        /// Second control point.
        c2: Vec2,
        /// End point.
        to: Vec2,
    },
    /// Close the current subpath back to its start.
    Close,
}

/// An ordered list of [`PathSegment`]s.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Path {
    /// The path's segments, in draw order.
    pub segments: Vec<PathSegment>,
}

impl Path {
    /// Creates an empty path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a `MoveTo` and returns `self` for chaining.
    #[must_use]
    pub fn move_to(mut self, x: f64, y: f64) -> Self {
        self.segments.push(PathSegment::MoveTo(Vec2::new(x, y)));
        self
    }

    /// Appends a `LineTo`.
    #[must_use]
    pub fn line_to(mut self, x: f64, y: f64) -> Self {
        self.segments.push(PathSegment::LineTo(Vec2::new(x, y)));
        self
    }

    /// Appends a quadratic Bézier.
    #[must_use]
    pub fn quad_to(mut self, cx: f64, cy: f64, x: f64, y: f64) -> Self {
        self.segments.push(PathSegment::QuadTo {
            ctrl: Vec2::new(cx, cy),
            to: Vec2::new(x, y),
        });
        self
    }

    /// Appends a cubic Bézier.
    #[must_use]
    pub fn cubic_to(mut self, c1x: f64, c1y: f64, c2x: f64, c2y: f64, x: f64, y: f64) -> Self {
        self.segments.push(PathSegment::CubicTo {
            c1: Vec2::new(c1x, c1y),
            c2: Vec2::new(c2x, c2y),
            to: Vec2::new(x, y),
        });
        self
    }

    /// Appends a `Close`.
    #[must_use]
    pub fn close(mut self) -> Self {
        self.segments.push(PathSegment::Close);
        self
    }

    /// Whether the path has no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_triangle_path() {
        let p = Path::new()
            .move_to(0.0, 0.0)
            .line_to(10.0, 0.0)
            .line_to(5.0, 8.0)
            .close();
        assert_eq!(p.segments.len(), 4);
        assert_eq!(p.segments[0], PathSegment::MoveTo(Vec2::new(0.0, 0.0)));
        assert_eq!(p.segments[3], PathSegment::Close);
        assert!(!p.is_empty());
    }

    #[test]
    fn empty_path() {
        assert!(Path::new().is_empty());
    }
}
