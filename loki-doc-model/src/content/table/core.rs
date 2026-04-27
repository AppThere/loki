// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Table, head, body, and foot types.
//!
//! Modelled on pandoc's `Table` type (pandoc-types ≥ 2.11):
//! `Table Attr Caption [ColSpec] TableHead [TableBody] TableFoot`.
//! TR 29166 §6.2.4 and §7.2.4.

use crate::content::attr::NodeAttr;
use crate::content::inline::Inline;
use crate::content::table::col::ColSpec;
use crate::content::table::row::Row;

/// The caption of a table.
///
/// Modelled on pandoc's `Caption = (Maybe ShortCaption, [Block])`.
/// For tables, the caption is a sequence of inline elements (no full blocks).
/// TR 29166 §7.2.4.
///
/// ODF: `table:title` / `table:desc`. OOXML: no native table caption;
/// often implemented as a styled paragraph immediately before or after
/// the table.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableCaption {
    /// A short form of the caption for use in lists of tables.
    pub short: Option<Vec<Inline>>,
    /// The full caption content.
    pub full: Vec<Inline>,
}

/// The head section of a table (header rows).
///
/// Modelled on pandoc's `TableHead = Attr [Row]`.
/// TR 29166 §7.2.4.
///
/// ODF: `table:table-header-rows`. OOXML: `w:tblHeader` property on rows.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableHead {
    /// Generic node attributes.
    pub attr: NodeAttr,
    /// The header rows.
    pub rows: Vec<Row>,
}

impl TableHead {
    /// Creates an empty table head.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            attr: NodeAttr::default(),
            rows: Vec::new(),
        }
    }
}

/// The foot section of a table (footer rows).
///
/// Modelled on pandoc's `TableFoot = Attr [Row]`.
/// TR 29166 §7.2.4.
///
/// ODF: no native concept; expressed as a repeated-header row or a final
/// `table:table-row`. OOXML: no native concept.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableFoot {
    /// Generic node attributes.
    pub attr: NodeAttr,
    /// The footer rows.
    pub rows: Vec<Row>,
}

impl TableFoot {
    /// Creates an empty table foot.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            attr: NodeAttr::default(),
            rows: Vec::new(),
        }
    }
}

/// One body section of a table.
///
/// Modelled on pandoc's `TableBody = Attr RowHeadColumns [Row] [Row]`
/// (head rows + body rows). For most documents there is exactly one body.
/// TR 29166 §7.2.4.
///
/// ODF: the main `table:table-row` sequence. OOXML: `w:tr` elements not
/// marked as header rows.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableBody {
    /// Generic node attributes.
    pub attr: NodeAttr,
    /// Head rows within the body (repeated header at section start).
    /// Corresponds to pandoc `rowHeadColumns` head rows.
    pub head_rows: Vec<Row>,
    /// The body rows.
    pub body_rows: Vec<Row>,
}

impl TableBody {
    /// Creates a body with only body rows (no per-body head rows).
    #[must_use]
    pub fn from_rows(rows: Vec<Row>) -> Self {
        Self {
            attr: NodeAttr::default(),
            head_rows: Vec::new(),
            body_rows: rows,
        }
    }
}

/// A complete table.
///
/// Modelled on pandoc's `Table` type (pandoc-types ≥ 2.11).
/// `Table Attr Caption [ColSpec] TableHead [TableBody] TableFoot`.
/// TR 29166 §6.2.4 and §7.2.4.
///
/// ODF: `table:table`. OOXML: `w:tbl`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Table {
    /// Generic node attributes (including table id and class).
    pub attr: NodeAttr,
    /// The table caption.
    pub caption: TableCaption,
    /// Column specifications, one per column in the table grid.
    pub col_specs: Vec<ColSpec>,
    /// The header row group.
    pub head: TableHead,
    /// One or more body row groups (usually one).
    pub bodies: Vec<TableBody>,
    /// The footer row group.
    pub foot: TableFoot,
}

impl Table {
    /// Returns the number of columns defined in the column grid.
    #[must_use]
    pub fn col_count(&self) -> usize {
        self.col_specs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::table::col::ColSpec;
    use crate::content::table::row::{Row, Cell};
    use loki_primitives::units::Points;

    #[test]
    fn table_two_by_two() {
        let cols = vec![
            ColSpec::fixed(Points::new(72.0)),
            ColSpec::fixed(Points::new(72.0)),
        ];
        let row1 = Row::new(vec![Cell::simple(vec![]), Cell::simple(vec![])]);
        let row2 = Row::new(vec![Cell::simple(vec![]), Cell::simple(vec![])]);
        let body = TableBody::from_rows(vec![row1, row2]);
        let table = Table {
            attr: NodeAttr::default(),
            caption: TableCaption::default(),
            col_specs: cols,
            head: TableHead::empty(),
            bodies: vec![body],
            foot: TableFoot::empty(),
        };
        assert_eq!(table.col_count(), 2);
        assert_eq!(table.bodies[0].body_rows.len(), 2);
        assert_eq!(table.bodies[0].body_rows[0].cells.len(), 2);
    }
}
