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

//! Table column specification types.
//!
//! Modelled on the pandoc `ColSpec = (Alignment, ColWidth)` type.
//! TR 29166 §6.2.4 and §7.2.4.

use loki_primitives::units::Points;

/// The width of a table column.
///
/// Modelled on pandoc's `ColWidth`. TR 29166 §7.2.4.
/// ODF: `style:column-width` or `style:rel-column-width`.
/// OOXML: `w:w` on `w:tc` (in twips) or `w:gridCol`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ColWidth {
    /// A fixed column width in points.
    Fixed(Points),
    /// A fractional share of the remaining table width.
    /// The value is a proportion (e.g. `1.0` for equal shares; `2.0` means
    /// twice as wide as a `1.0` column). Corresponds to pandoc `ColWidthDefault`.
    /// ODF: `style:rel-column-width`. OOXML: proportional grid columns.
    Proportional(f32),
    /// Column width is determined by content. Corresponds to pandoc
    /// `ColWidthDefault` when no explicit width is given.
    Default,
}

/// Horizontal alignment of content within a table column or cell.
///
/// Modelled on pandoc's `Alignment`. TR 29166 §6.2.4.
/// ODF: `fo:text-align` on `style:table-cell-properties`.
/// OOXML: `w:jc` on `w:tcPr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ColAlignment {
    /// No alignment specified; inherits from table or paragraph style.
    #[default]
    Default,
    /// Left-aligned (start-aligned in LTR).
    Left,
    /// Right-aligned (end-aligned in LTR).
    Right,
    /// Centered.
    Center,
}

/// The specification for a single table column.
///
/// Modelled on pandoc's `ColSpec = (Alignment, ColWidth)`.
/// TR 29166 §7.2.4.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColSpec {
    /// The default horizontal alignment for cells in this column.
    pub alignment: ColAlignment,
    /// The width of the column.
    pub width: ColWidth,
}

impl ColSpec {
    /// Creates a [`ColSpec`] with default alignment and a fixed width.
    #[must_use]
    pub fn fixed(width: Points) -> Self {
        Self {
            alignment: ColAlignment::Default,
            width: ColWidth::Fixed(width),
        }
    }

    /// Creates a [`ColSpec`] with default alignment and proportional width.
    #[must_use]
    pub fn proportional(share: f32) -> Self {
        Self {
            alignment: ColAlignment::Default,
            width: ColWidth::Proportional(share),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn col_spec_fixed() {
        let spec = ColSpec::fixed(Points::new(72.0));
        assert!(matches!(spec.width, ColWidth::Fixed(_)));
        assert_eq!(spec.alignment, ColAlignment::Default);
    }

    #[test]
    fn col_spec_proportional() {
        let spec = ColSpec::proportional(2.0);
        assert!(matches!(spec.width, ColWidth::Proportional(v) if (v - 2.0).abs() < f32::EPSILON));
    }
}
