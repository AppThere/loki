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

/// Edge insets representing margin, padding, or border widths.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Insets<U> {
    /// Top inset
    pub top: Length<U>,
    /// Right inset
    pub right: Length<U>,
    /// Bottom inset
    pub bottom: Length<U>,
    /// Left inset
    pub left: Length<U>,
}

impl<U: Copy> Insets<U> {
    /// Constructs insets.
    #[must_use]
    pub fn new(
        top: Length<U>,
        right: Length<U>,
        bottom: Length<U>,
        left: Length<U>,
    ) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Forms a uniform inset.
    #[must_use]
    pub fn uniform(value: Length<U>) -> Self {
        Self::new(value, value, value, value)
    }

    /// Forms a symmetrically proportioned inset.
    #[must_use]
    pub fn symmetric(vertical: Length<U>, horizontal: Length<U>) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }

    /// Combined horizontal component total
    #[must_use]
    pub fn horizontal(self) -> Length<U> {
        self.left + self.right
    }

    /// Combined vertical component total
    #[must_use]
    pub fn vertical(self) -> Length<U> {
        self.top + self.bottom
    }
}
