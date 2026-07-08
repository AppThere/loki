// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Named table style definition.
//!
//! TR 29166 §6.2.4 and §7.2.4 describe the ODF and OOXML table models.
//! ODF uses `style:style style:family="table"`; OOXML uses
//! `w:style w:type="table"`.

use crate::content::attr::ExtensionBag;
use crate::style::catalog::StyleId;
use crate::style::props::border::Border;
use indexmap::IndexMap;
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

/// Table width specification.
///
/// TR 29166 §6.2.4. ODF `style:width` or `style:rel-width`;
/// OOXML `w:tblW`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TableWidth {
    /// An absolute width in points.
    Absolute(Points),
    /// A percentage of the page text area width.
    Percent(f32),
    /// The table width is determined by its content (auto).
    Auto,
}

/// Horizontal alignment of a table on the page.
///
/// ODF `table:align`; OOXML `w:jc` on `w:tblPr`.
/// TR 29166 §6.2.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TableAlignment {
    /// Table is left-aligned (the default).
    #[default]
    Left,
    /// Table is centered.
    Center,
    /// Table is right-aligned.
    Right,
}

/// Table-level formatting properties.
///
/// TR 29166 §6.2.4 "Table formatting" and §7.2.4.
/// ODF: `style:table-properties`. OOXML: `w:tblPr`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableProps {
    /// The table width. `None` inherits from the style default.
    pub width: Option<TableWidth>,
    /// Horizontal alignment of the table block on the page.
    pub alignment: Option<TableAlignment>,
    /// Default cell padding (inside border) in points.
    pub cell_padding: Option<Points>,
    /// Cell spacing (border collapse separation). `None` = collapsed.
    pub cell_spacing: Option<Points>,
    /// Default outside border for all table edges.
    pub border: Option<Border>,
    /// Background color of the table.
    pub background_color: Option<DocumentColor>,
    /// Number of rows in each horizontal band. OOXML
    /// `w:tblStyleRowBandSize`; `None` = the default of 1.
    #[cfg_attr(feature = "serde", serde(default))]
    pub row_band_size: Option<u32>,
    /// Number of columns in each vertical band. OOXML
    /// `w:tblStyleColBandSize`; `None` = the default of 1.
    #[cfg_attr(feature = "serde", serde(default))]
    pub col_band_size: Option<u32>,
}

/// A conditional region of a table that a style can format differently.
///
/// OOXML `w:tblStylePr w:type="…"` (TR 29166 §7.2.4). The twelve
/// non-`WholeTable` variants correspond one-to-one with the OOXML
/// `w:cnfStyle`/`w:tblLook` region flags. ODF has no direct equivalent;
/// LibreOffice synthesises these from separate cell styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum TableRegion {
    /// The base formatting applied to every cell (`wholeTable`).
    WholeTable,
    /// The header row (`firstRow`).
    FirstRow,
    /// The total/footer row (`lastRow`).
    LastRow,
    /// The leading column (`firstCol`).
    FirstColumn,
    /// The trailing column (`lastCol`).
    LastColumn,
    /// Odd horizontal bands (`band1Horz`).
    Band1Horz,
    /// Even horizontal bands (`band2Horz`).
    Band2Horz,
    /// Odd vertical bands (`band1Vert`).
    Band1Vert,
    /// Even vertical bands (`band2Vert`).
    Band2Vert,
    /// The top-left corner cell (`nwCell`).
    NwCell,
    /// The top-right corner cell (`neCell`).
    NeCell,
    /// The bottom-left corner cell (`swCell`).
    SwCell,
    /// The bottom-right corner cell (`seCell`).
    SeCell,
}

/// The formatting a table style applies to one [`TableRegion`].
///
/// Only cell shading is modeled today; borders and character formatting
/// are future work (Spec 05, 4a.3). OOXML `w:tblStylePr` child props.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableConditionalFormat {
    /// Cell background (shading) for cells in this region.
    pub background_color: Option<DocumentColor>,
}

/// Which conditional regions of a table style are active for one table
/// instance.
///
/// OOXML `w:tblLook` (TR 29166 §7.2.4). Each flag enables the matching
/// [`TableRegion`] family; when a banding flag is off, the band regions
/// are suppressed and cells fall through to `WholeTable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableLook {
    /// Apply special formatting to the header row.
    pub first_row: bool,
    /// Apply special formatting to the total/footer row.
    pub last_row: bool,
    /// Apply special formatting to the leading column.
    pub first_column: bool,
    /// Apply special formatting to the trailing column.
    pub last_column: bool,
    /// Apply row-banding (horizontal stripes).
    pub horizontal_banding: bool,
    /// Apply column-banding (vertical stripes).
    pub vertical_banding: bool,
}

impl Default for TableLook {
    /// Word's default `w:tblLook` of `04A0`: header row, first column, and
    /// row banding on; footer row, last column, and column banding off.
    fn default() -> Self {
        Self {
            first_row: true,
            last_row: false,
            first_column: true,
            last_column: false,
            horizontal_banding: true,
            vertical_banding: false,
        }
    }
}

/// A named table style.
///
/// TR 29166 §7.2.4 (Table XML structure comparison).
/// ODF: `style:style style:family="table"`.
/// OOXML: `w:style w:type="table"`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableStyle {
    /// The unique identifier used to reference this style.
    pub id: StyleId,

    /// A human-readable display name.
    pub display_name: Option<String>,

    /// The parent style identifier.
    pub parent: Option<StyleId>,

    /// Table-level formatting properties.
    pub table_props: TableProps,

    /// Conditional (region-specific) formatting. Keyed by [`TableRegion`];
    /// an absent region inherits from `WholeTable` (or nothing). OOXML
    /// `w:tblStylePr`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub conditional: IndexMap<TableRegion, TableConditionalFormat>,

    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_style_default_props() {
        let style = TableStyle {
            id: StyleId("TableGrid".into()),
            display_name: Some("Table Grid".into()),
            parent: None,
            table_props: TableProps::default(),
            conditional: IndexMap::new(),
            extensions: ExtensionBag::default(),
        };
        assert!(style.table_props.width.is_none());
        assert!(style.table_props.border.is_none());
        assert!(style.conditional.is_empty());
    }

    #[test]
    fn default_table_look_matches_word_04a0() {
        let look = TableLook::default();
        assert!(look.first_row);
        assert!(look.first_column);
        assert!(look.horizontal_banding);
        assert!(!look.last_row);
        assert!(!look.last_column);
        assert!(!look.vertical_banding);
    }
}
