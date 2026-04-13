// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

use crate::units::Length;
use super::size::Size;
use std::ops::{Add, Sub};

/// A point in 2D space.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point<U> {
    /// X coordinate
    pub x: Length<U>,
    /// Y coordinate
    pub y: Length<U>,
}

impl<U: Copy> Point<U> {
    /// Creates a new point.
    #[must_use]
    pub fn new(x: Length<U>, y: Length<U>) -> Self {
        Self { x, y }
    }

    /// Returns the origin point (0, 0).
    #[must_use]
    pub fn origin() -> Self {
        Self::new(Length::zero(), Length::zero())
    }

    /// Translates the point by given delta values.
    #[must_use]
    pub fn translate(self, dx: Length<U>, dy: Length<U>) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    /// Calculates Euclidean distance to another point.
    #[must_use]
    pub fn distance_to(self, other: Self) -> f64 {
        let dx = (self.x - other.x).value();
        let dy = (self.y - other.y).value();
        dx.hypot(dy)
    }
}

impl<U: Copy> Add<Size<U>> for Point<U> {
    type Output = Point<U>;

    fn add(self, rhs: Size<U>) -> Self::Output {
        Self::new(self.x + rhs.width, self.y + rhs.height)
    }
}

impl<U: Copy> Sub<Point<U>> for Point<U> {
    type Output = Size<U>;

    fn sub(self, rhs: Point<U>) -> Self::Output {
        Size::new(self.x - rhs.x, self.y - rhs.y)
    }
}
