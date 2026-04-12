// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Geometry primitives for layout space.
//!
//! All types work in `f32` points. They are intentionally separate from
//! `loki_primitives::geometry` (which uses typed `Length<U>` units) to avoid
//! unit confusion in layout arithmetic.

/// A 2D point in layout space (points, `f32`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutPoint {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

impl LayoutPoint {
    /// Creates a new point.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A 2D size in layout space (points, `f32`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutSize {
    /// Horizontal extent.
    pub width: f32,
    /// Vertical extent.
    pub height: f32,
}

impl LayoutSize {
    /// Creates a new size.
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// An axis-aligned rectangle in layout space (points, `f32`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutRect {
    /// Top-left corner.
    pub origin: LayoutPoint,
    /// Dimensions.
    pub size: LayoutSize,
}

impl LayoutRect {
    /// Creates a rectangle from its components.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: LayoutPoint { x, y },
            size: LayoutSize { width, height },
        }
    }

    /// X coordinate of the left edge.
    pub fn x(&self) -> f32 {
        self.origin.x
    }

    /// Y coordinate of the top edge.
    pub fn y(&self) -> f32 {
        self.origin.y
    }

    /// Width of the rectangle.
    pub fn width(&self) -> f32 {
        self.size.width
    }

    /// Height of the rectangle.
    pub fn height(&self) -> f32 {
        self.size.height
    }

    /// X coordinate of the right edge.
    pub fn max_x(&self) -> f32 {
        self.origin.x + self.size.width
    }

    /// Y coordinate of the bottom edge.
    pub fn max_y(&self) -> f32 {
        self.origin.y + self.size.height
    }

    /// Returns `true` if `p` lies within (or on the boundary of) this rect.
    pub fn contains_point(&self, p: LayoutPoint) -> bool {
        p.x >= self.origin.x
            && p.x <= self.max_x()
            && p.y >= self.origin.y
            && p.y <= self.max_y()
    }

    /// Returns `true` if this rectangle overlaps with `other`.
    ///
    /// Rectangles that merely touch at an edge are considered to intersect.
    pub fn intersects(&self, other: &Self) -> bool {
        self.origin.x <= other.max_x()
            && self.max_x() >= other.origin.x
            && self.origin.y <= other.max_y()
            && self.max_y() >= other.origin.y
    }
}

/// Insets (padding or margin) in layout space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutInsets {
    /// Top inset.
    pub top: f32,
    /// Right inset.
    pub right: f32,
    /// Bottom inset.
    pub bottom: f32,
    /// Left inset.
    pub left: f32,
}

impl LayoutInsets {
    /// Creates insets where all four sides share the same value.
    pub fn uniform(v: f32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }

    /// Sum of left and right insets.
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Sum of top and bottom insets.
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_max_xy() {
        let r = LayoutRect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.max_x(), 110.0);
        assert_eq!(r.max_y(), 70.0);
    }

    #[test]
    fn rect_contains_point() {
        let r = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains_point(LayoutPoint::new(5.0, 5.0)));
        assert!(r.contains_point(LayoutPoint::new(0.0, 0.0)));
        assert!(r.contains_point(LayoutPoint::new(10.0, 10.0)));
        assert!(!r.contains_point(LayoutPoint::new(10.1, 5.0)));
        assert!(!r.contains_point(LayoutPoint::new(-0.1, 5.0)));
    }

    #[test]
    fn rect_intersects() {
        let a = LayoutRect::new(0.0, 0.0, 10.0, 10.0);
        let b = LayoutRect::new(5.0, 5.0, 10.0, 10.0);
        let c = LayoutRect::new(10.0, 0.0, 10.0, 10.0); // touches edge
        let d = LayoutRect::new(10.1, 0.0, 10.0, 10.0); // no overlap
        assert!(a.intersects(&b));
        assert!(a.intersects(&c));
        assert!(!a.intersects(&d));
    }

    #[test]
    fn insets_horizontal_vertical() {
        let ins = LayoutInsets { top: 1.0, right: 2.0, bottom: 3.0, left: 4.0 };
        assert_eq!(ins.horizontal(), 6.0);
        assert_eq!(ins.vertical(), 4.0);
    }

    #[test]
    fn insets_uniform() {
        let ins = LayoutInsets::uniform(5.0);
        assert_eq!(ins.top, 5.0);
        assert_eq!(ins.right, 5.0);
        assert_eq!(ins.bottom, 5.0);
        assert_eq!(ins.left, 5.0);
    }
}
