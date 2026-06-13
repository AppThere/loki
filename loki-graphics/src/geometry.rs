// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Lightweight f64 geometry for vector graphics.
//!
//! All coordinates and lengths are in **points** (1/72 inch), the document
//! standard. Vector graphics use uniform `f64` (rather than the typed
//! `loki_primitives::units::Length`) so path data, transforms, and gradients
//! interoperate without conversion friction — the same convention used by
//! `kurbo`/`lyon` and what Iris Draw will want.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A 2D point or vector, in points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Vec2 {
    /// Horizontal coordinate (points).
    pub x: f64,
    /// Vertical coordinate (points).
    pub y: f64,
}

impl Vec2 {
    /// The origin `(0, 0)`.
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    /// Creates a point from coordinates in points.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Returns this point translated by `(dx, dy)`.
    #[must_use]
    pub fn translate(self, dx: f64, dy: f64) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
}

/// An axis-aligned rectangle, in points: top-left origin plus extent.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RectF {
    /// Left edge (points).
    pub x: f64,
    /// Top edge (points).
    pub y: f64,
    /// Width (points); should be non-negative.
    pub width: f64,
    /// Height (points); should be non-negative.
    pub height: f64,
}

impl RectF {
    /// Creates a rectangle from origin and extent.
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The right edge (`x + width`).
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// The bottom edge (`y + height`).
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// The geometric center.
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// The top-left origin as a [`Vec2`].
    pub fn origin(&self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    /// Whether `p` lies within the rectangle (edges inclusive).
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
}

/// A width/height extent, in points.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Size {
    /// Width (points).
    pub width: f64,
    /// Height (points).
    pub height: f64,
}

impl Size {
    /// Creates an extent from width and height in points.
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_edges_and_center() {
        let r = RectF::new(10.0, 20.0, 100.0, 40.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 60.0);
        assert_eq!(r.center(), Vec2::new(60.0, 40.0));
        assert_eq!(r.origin(), Vec2::new(10.0, 20.0));
    }

    #[test]
    fn rect_contains() {
        let r = RectF::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(Vec2::new(5.0, 5.0)));
        assert!(r.contains(Vec2::new(0.0, 0.0)));
        assert!(r.contains(Vec2::new(10.0, 10.0)));
        assert!(!r.contains(Vec2::new(11.0, 5.0)));
    }

    #[test]
    fn vec_translate() {
        assert_eq!(Vec2::ZERO.translate(3.0, 4.0), Vec2::new(3.0, 4.0));
    }
}
