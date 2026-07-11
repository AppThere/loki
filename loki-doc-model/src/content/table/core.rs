// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table, head, body, and foot types.
//!
//! Modelled on pandoc's `Table` type (pandoc-types ≥ 2.11):
//! `Table Attr Caption [ColSpec] TableHead [TableBody] TableFoot`.
//! TR 29166 §6.2.4 and §7.2.4.

use crate::content::attr::NodeAttr;
use crate::content::inline::Inline;
use crate::content::table::col::{ColSpec, TableWidth};
use crate::content::table::row::Row;

/// Class on a [`Table`]'s [`NodeAttr`] marking it as **fixed layout** — column
/// widths come from the grid (`w:tblGrid`/gridCol) and are honoured exactly,
/// even when they sum to more or less than the table width (the table then
/// overflows or underfills). OOXML `w:tblLayout w:type="fixed"`. Absent ⇒
/// autofit: columns are resized to fit the table width.
pub const TABLE_FIXED_LAYOUT_CLASS: &str = "table-fixed-layout";

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
    /// Overall table width. `None` means the renderer decides.
    /// ODF: `style:width` on table style; OOXML: `w:tblW`.
    pub width: Option<TableWidth>,
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

    /// The referenced table style's id — OOXML `w:tblStyle` / ODF
    /// `table:style-name` — stored in [`NodeAttr`]'s `"style"` key (the same
    /// convention a [`Block::Heading`] uses). `None` when there is no named
    /// style. The style supplies table-level defaults and (future) banding /
    /// conditional-region formatting.
    ///
    /// [`Block::Heading`]: crate::content::block::Block::Heading
    #[must_use]
    pub fn style_name(&self) -> Option<&str> {
        self.attr
            .kv
            .iter()
            .find(|(k, _)| k == "style")
            .map(|(_, v)| v.as_str())
    }

    /// Sets (or, with `None`, clears) the referenced table style id.
    pub fn set_style_name(&mut self, id: Option<String>) {
        self.attr.kv.retain(|(k, _)| k != "style");
        if let Some(id) = id {
            self.attr.kv.push(("style".to_string(), id));
        }
    }

    /// The encoded OOXML `w:tblLook` region flags for this table instance,
    /// stored in [`NodeAttr`]'s `"tbllook"` key (see `TableLook::encode_attr`).
    /// `None` ⇒ consumers assume the format default. Selects which of the
    /// referenced style's conditional regions apply to this table. Stored as an
    /// opaque string so `content` need not depend on the `style` module.
    #[must_use]
    pub fn table_look_code(&self) -> Option<&str> {
        self.attr
            .kv
            .iter()
            .find(|(k, _)| k == "tbllook")
            .map(|(_, v)| v.as_str())
    }

    /// Sets (or, with `None`, clears) the encoded `w:tblLook` region flags.
    pub fn set_table_look_code(&mut self, code: Option<String>) {
        self.attr.kv.retain(|(k, _)| k != "tbllook");
        if let Some(code) = code {
            self.attr.kv.push(("tbllook".to_string(), code));
        }
    }

    /// Builds a `rows` × `cols` table of empty paragraph cells with evenly
    /// proportioned columns — the shape the editor's Insert → Table control
    /// creates. Each cell holds one empty `Block::Para` so it is immediately
    /// editable via a `BlockPath`. `rows` and `cols` are clamped to at least 1.
    #[must_use]
    pub fn grid(rows: usize, cols: usize) -> Self {
        use crate::content::block::Block;
        use crate::content::table::row::{Cell, Row};
        let rows = rows.max(1);
        let cols = cols.max(1);
        let col_specs = (0..cols).map(|_| ColSpec::proportional(1.0)).collect();
        let body_rows = (0..rows)
            .map(|_| {
                Row::new(
                    (0..cols)
                        .map(|_| Cell::simple(vec![Block::Para(Vec::new())]))
                        .collect(),
                )
            })
            .collect();
        Self {
            attr: NodeAttr::default(),
            caption: TableCaption::default(),
            width: None,
            col_specs,
            head: TableHead::empty(),
            bodies: vec![TableBody::from_rows(body_rows)],
            foot: TableFoot::empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::table::col::ColSpec;
    use crate::content::table::row::{Cell, Row};
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
            width: None,
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
