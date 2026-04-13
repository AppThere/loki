// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! ODF table model types.
//!
//! Mirrors the `table:table` element tree as defined in ODF 1.3 §9.
//! Only the structural elements needed for import are modelled here;
//! column-spanning, row-spanning, and covered cells are tracked via
//! [`OdfTableCell::col_span`], [`OdfTableCell::row_span`], and
//! [`OdfTableCell::is_covered`].

use super::paragraph::OdfParagraph;

/// An ODF table (`table:table`). ODF 1.3 §9.1.
#[derive(Debug, Clone)]
pub(crate) struct OdfTable {
    /// `table:name` — unique name within the document.
    pub name: Option<String>,
    /// `table:style-name` — table-level style reference.
    pub style_name: Option<String>,
    /// Column definitions, in document order.
    pub col_defs: Vec<OdfTableColDef>,
    /// Rows, in document order.
    pub rows: Vec<OdfTableRow>,
}

/// A column definition (`table:table-column`). ODF 1.3 §9.2.
#[derive(Debug, Clone)]
pub(crate) struct OdfTableColDef {
    /// `table:style-name` — column style reference.
    pub style_name: Option<String>,
    /// `table:number-columns-repeated` — how many contiguous columns share
    /// this definition (defaults to 1).
    pub columns_repeated: u32,
}

/// A table row (`table:table-row`). ODF 1.3 §9.3.
#[derive(Debug, Clone)]
pub(crate) struct OdfTableRow {
    /// `table:style-name` — row style reference.
    pub style_name: Option<String>,
    /// Cells in this row, in document order. May include covered cells.
    pub cells: Vec<OdfTableCell>,
}

/// A table cell (`table:table-cell` or `table:covered-table-cell`).
///
/// ODF 1.3 §9.4 (`table:table-cell`), §9.5 (`table:covered-table-cell`).
/// Covered cells exist in the grid to satisfy the row-width invariant but
/// carry no content of their own.
#[derive(Debug, Clone)]
pub(crate) struct OdfTableCell {
    /// `table:style-name` — cell style reference.
    pub style_name: Option<String>,
    /// `table:number-columns-spanned` — horizontal span (defaults to 1).
    pub col_span: u32,
    /// `table:number-rows-spanned` — vertical span (defaults to 1).
    pub row_span: u32,
    /// `true` when this cell is a `table:covered-table-cell`.
    pub is_covered: bool,
    /// `office:value-type` — e.g. `"string"`, `"float"`, `"date"`.
    pub value_type: Option<String>,
    /// Text paragraphs inside this cell. ODF 1.3 §9.4.
    pub paragraphs: Vec<OdfParagraph>,
}
