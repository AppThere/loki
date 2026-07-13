// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table row and cell types.
//!
//! Modelled on pandoc's `Row`, `Cell`, and related types.
//! TR 29166 §6.2.4 and §7.2.4.

use crate::content::attr::NodeAttr;
use crate::content::block::Block;
use crate::content::table::col::ColAlignment;
use crate::style::props::border::Border;
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

/// Vertical alignment of content within a table cell.
///
/// TR 29166 §6.2.4. ODF `style:vertical-align`; OOXML `w:vAlign`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum CellVerticalAlign {
    /// Content is aligned to the top of the cell (the default).
    #[default]
    Top,
    /// Content is centered vertically.
    Middle,
    /// Content is aligned to the bottom.
    Bottom,
}

/// Text direction within a table cell.
///
/// OOXML §17.4.87 `w:textDirection @w:val`.
/// ODF `style:writing-mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum CellTextDirection {
    /// Horizontal left-to-right (default).
    LrTb,
    /// Vertical top-to-bottom, right-to-left (rotated 90° clockwise).
    TbRl,
    /// Vertical top-to-bottom, left-to-right (rotated 90° counter-clockwise).
    TbLr,
    /// Vertical bottom-to-top, left-to-right.
    BtLr,
}

/// Formatting properties for a table cell.
///
/// TR 29166 §6.2.4 "Table cell formatting".
/// ODF: `style:table-cell-properties`. OOXML: `w:tcPr`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CellProps {
    /// Background fill color. ODF `fo:background-color`; OOXML `w:shd`.
    pub background_color: Option<DocumentColor>,
    /// Top border.
    pub border_top: Option<Border>,
    /// Bottom border.
    pub border_bottom: Option<Border>,
    /// Start (left in LTR) border.
    pub border_left: Option<Border>,
    /// End (right in LTR) border.
    pub border_right: Option<Border>,
    /// Padding inside the top border.
    pub padding_top: Option<Points>,
    /// Padding inside the bottom border.
    pub padding_bottom: Option<Points>,
    /// Padding inside the start border.
    pub padding_left: Option<Points>,
    /// Padding inside the end border.
    pub padding_right: Option<Points>,
    /// Vertical alignment of the cell content.
    pub vertical_align: Option<CellVerticalAlign>,
    /// Text flow direction within the cell. `None` = horizontal LrTb (default).
    pub text_direction: Option<CellTextDirection>,
}

/// A single table cell.
///
/// Modelled on pandoc's `Cell` type.
/// `Cell = Attr Alignment RowSpan ColSpan [Block]`.
/// TR 29166 §7.2.4.
///
/// ODF: `table:table-cell`. OOXML: `w:tc`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Cell {
    /// Generic node attributes.
    pub attr: NodeAttr,
    /// Horizontal alignment override for this cell (overrides column default).
    pub alignment: ColAlignment,
    /// Number of rows this cell spans. 1 = no spanning.
    pub row_span: u32,
    /// Number of columns this cell spans. 1 = no spanning.
    pub col_span: u32,
    /// The cell content as a sequence of blocks.
    pub blocks: Vec<Block>,
    /// Cell-level formatting properties.
    pub props: CellProps,
}

impl Cell {
    /// The cell's encoded `w:cnfStyle` region mask (ECMA-376 §17.4.7), if
    /// the importer recorded one — see `style::table_cnf::TableCnf`. Stored
    /// as an opaque code in the cell attr (`"cnf"`), the `"tbllook"` idiom.
    #[must_use]
    pub fn cnf_code(&self) -> Option<&str> {
        self.attr
            .kv
            .iter()
            .find(|(k, _)| k == "cnf")
            .map(|(_, v)| v.as_str())
    }

    /// Sets (or, with `None`, clears) the encoded `w:cnfStyle` mask.
    pub fn set_cnf_code(&mut self, code: Option<String>) {
        self.attr.kv.retain(|(k, _)| k != "cnf");
        if let Some(code) = code {
            self.attr.kv.push(("cnf".to_string(), code));
        }
    }

    /// Creates a simple cell spanning one row and one column.
    #[must_use]
    pub fn simple(blocks: Vec<Block>) -> Self {
        Self {
            attr: NodeAttr::default(),
            alignment: ColAlignment::Default,
            row_span: 1,
            col_span: 1,
            blocks,
            props: CellProps::default(),
        }
    }
}

/// A table row.
///
/// Modelled on pandoc's `Row = Attr [Cell]`.
/// TR 29166 §7.2.4.
///
/// ODF: `table:table-row`. OOXML: `w:tr`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Row {
    /// Generic node attributes.
    pub attr: NodeAttr,
    /// The cells in this row.
    pub cells: Vec<Cell>,
}

impl Row {
    /// Creates a row from a list of cells.
    #[must_use]
    pub fn new(cells: Vec<Cell>) -> Self {
        Self {
            attr: NodeAttr::default(),
            cells,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_simple_no_span() {
        let cell = Cell::simple(vec![]);
        assert_eq!(cell.row_span, 1);
        assert_eq!(cell.col_span, 1);
        assert!(cell.blocks.is_empty());
    }

    #[test]
    fn row_from_cells() {
        let row = Row::new(vec![Cell::simple(vec![]), Cell::simple(vec![])]);
        assert_eq!(row.cells.len(), 2);
    }
}
