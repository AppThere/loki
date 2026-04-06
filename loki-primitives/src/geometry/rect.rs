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

use crate::units::Length;
use super::{point::Point, size::Size, insets::Insets};

/// An axis-aligned rectangle defined by an origin and size.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rect<U> {
    /// Origin of rect
    pub origin: Point<U>,
    /// Size geometry 
    pub size: Size<U>,
}

impl<U: Copy> Rect<U> {
    /// Factory for creating rect via size/origin definitions.
    #[must_use]
    pub fn new(origin: Point<U>, size: Size<U>) -> Self {
        Self { origin, size }
    }

    /// Factory method spanning left right top bottoms limits.
    #[must_use]
    pub fn from_ltrb(
        left: Length<U>,
        top: Length<U>,
        right: Length<U>,
        bottom: Length<U>,
    ) -> Self {
        Self::new(
            Point::new(left, top),
            Size::new(right - left, bottom - top),
        )
    }

    /// Returns point min
    #[must_use]
    pub fn min_x(self) -> Length<U> {
        self.origin.x
    }
    
    /// Returns point min
    #[must_use]
    pub fn min_y(self) -> Length<U> {
        self.origin.y
    }
    
    /// Returns point max
    #[must_use]
    pub fn max_x(self) -> Length<U> {
        self.origin.x + self.size.width
    }
    
    /// Returns point max
    #[must_use]
    pub fn max_y(self) -> Length<U> {
        self.origin.y + self.size.height
    }

    /// Produces its central coordinates
    #[must_use]
    pub fn center(self) -> Point<U> {
        Point::new(
            self.origin.x + (self.size.width / 2.0),
            self.origin.y + (self.size.height / 2.0),
        )
    }

    /// Detects inclusion
    #[must_use]
    pub fn contains_point(self, p: Point<U>) -> bool {
        p.x.value() >= self.min_x().value()
            && p.x.value() < self.max_x().value()
            && p.y.value() >= self.min_y().value()
            && p.y.value() < self.max_y().value()
    }

    /// Tests for crossing
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.min_x().value() < other.max_x().value()
            && self.max_x().value() > other.min_x().value()
            && self.min_y().value() < other.max_y().value()
            && self.max_y().value() > other.min_y().value()
    }

    /// Merging overlapping segments or returning nothing 
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }
        let min_x = self.min_x().max(other.min_x());
        let min_y = self.min_y().max(other.min_y());
        let max_x = self.max_x().min(other.max_x());
        let max_y = self.max_y().min(other.max_y());
        Some(Self::from_ltrb(min_x, min_y, max_x, max_y))
    }

    /// Creates bounding rect for both rects.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let min_x = self.min_x().min(other.min_x());
        let min_y = self.min_y().min(other.min_y());
        let max_x = self.max_x().max(other.max_x());
        let max_y = self.max_y().max(other.max_y());
        Self::from_ltrb(min_x, min_y, max_x, max_y)
    }

    /// Insets (shrinks) rect
    #[must_use]
    pub fn inset(self, insets: Insets<U>) -> Self {
        Self::from_ltrb(
            self.min_x() + insets.left,
            self.min_y() + insets.top,
            self.max_x() - insets.right,
            self.max_y() - insets.bottom,
        )
    }

    /// Outsets (expands) rect
    #[must_use]
    pub fn outset(self, insets: Insets<U>) -> Self {
        Self::from_ltrb(
            self.min_x() - insets.left,
            self.min_y() - insets.top,
            self.max_x() + insets.right,
            self.max_y() + insets.bottom,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Pt;
    use approx::assert_relative_eq;

    #[test]
    fn test_rect_intersection() {
        let r1 = Rect::<Pt>::from_ltrb(Length::new(0.0), Length::new(0.0), Length::new(10.0), Length::new(10.0));
        let r2 = Rect::<Pt>::from_ltrb(Length::new(5.0), Length::new(5.0), Length::new(15.0), Length::new(15.0));

        let int = r1.intersection(r2).unwrap();
        assert_relative_eq!(int.min_x().value(), 5.0);
        assert_relative_eq!(int.min_y().value(), 5.0);
        assert_relative_eq!(int.max_x().value(), 10.0);
        assert_relative_eq!(int.max_y().value(), 10.0);

        let disjoint = Rect::<Pt>::from_ltrb(Length::new(20.0), Length::new(20.0), Length::new(30.0), Length::new(30.0));
        assert!(r1.intersection(disjoint).is_none());
    }

    #[test]
    fn test_rect_union() {
        let r1 = Rect::<Pt>::from_ltrb(Length::new(0.0), Length::new(0.0), Length::new(10.0), Length::new(10.0));
        let r2 = Rect::<Pt>::from_ltrb(Length::new(5.0), Length::new(5.0), Length::new(15.0), Length::new(15.0));

        let u = r1.union(r2);
        assert_relative_eq!(u.min_x().value(), 0.0);
        assert_relative_eq!(u.min_y().value(), 0.0);
        assert_relative_eq!(u.max_x().value(), 15.0);
        assert_relative_eq!(u.max_y().value(), 15.0);
    }
    
    #[test]
    fn test_rect_inset() {
        let r1 = Rect::<Pt>::from_ltrb(Length::new(0.0), Length::new(0.0), Length::new(10.0), Length::new(10.0));
        let insets = Insets::<Pt>::new(Length::new(1.0), Length::new(2.0), Length::new(3.0), Length::new(4.0));
        let shrunk = r1.inset(insets);
        assert_relative_eq!(shrunk.min_x().value(), 4.0);
        assert_relative_eq!(shrunk.min_y().value(), 1.0);
        assert_relative_eq!(shrunk.max_x().value(), 8.0);
        assert_relative_eq!(shrunk.max_y().value(), 7.0);
    }

    #[test]
    fn test_contains_point() {
        let r = Rect::<Pt>::from_ltrb(Length::new(0.0), Length::new(0.0), Length::new(10.0), Length::new(10.0));
        assert!(r.contains_point(Point::new(Length::new(5.0), Length::new(5.0))));
        assert!(r.contains_point(Point::new(Length::new(0.0), Length::new(0.0)))); // corner inclusive minimum
        assert!(!r.contains_point(Point::new(Length::new(10.0), Length::new(10.0)))); // corner exclusive bound
        assert!(!r.contains_point(Point::new(Length::new(15.0), Length::new(5.0))));
    }
}
