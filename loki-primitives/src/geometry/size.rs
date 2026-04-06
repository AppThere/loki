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

/// Dimension in 2D space.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Size<U> {
    /// Width of size
    pub width: Length<U>,
    /// Height of size
    pub height: Length<U>,
}

impl<U: Copy> Size<U> {
    /// Constructs a size.
    #[must_use]
    pub fn new(width: Length<U>, height: Length<U>) -> Self {
        Self { width, height }
    }

    /// Extends size
    #[must_use]
    pub fn zero() -> Self {
        Self::new(Length::zero(), Length::zero())
    }

    /// Evaluates area as real non typed value
    #[must_use]
    pub fn area(self) -> f64 {
        self.width.value() * self.height.value()
    }

    /// Returns whether this dimension has non-positive area.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width.value() <= 0.0 || self.height.value() <= 0.0
    }

    /// Uniform scale modifier
    #[must_use]
    pub fn scale(self, factor: f64) -> Self {
        Self::new(self.width * factor, self.height * factor)
    }

    /// Ensure other size fits into this one.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.width.value() >= other.width.value() && self.height.value() >= other.height.value()
    }
}
