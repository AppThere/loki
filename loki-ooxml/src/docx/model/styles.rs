// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Intermediate model for `word/styles.xml`.
//!
//! Mirrors ECMA-376 §17.7 (document styles).

use super::paragraph::{DocxBorderEdge, DocxPPr, DocxRPr};

/// Top-level model for `w:styles` (ECMA-376 §17.7.4.18).
#[derive(Debug, Clone, Default)]
pub struct DocxStyles {
    /// Document-default run properties.
    pub default_rpr: Option<DocxRPr>,
    /// Document-default paragraph properties.
    pub default_ppr: Option<DocxPPr>,
    /// All style definitions.
    pub styles: Vec<DocxStyle>,
}

/// The type of a style definition (ECMA-376 §17.18.83).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocxStyleType {
    /// A paragraph style (`w:type="paragraph"`).
    Paragraph,
    /// A character style (`w:type="character"`).
    Character,
    /// A table style (`w:type="table"`).
    Table,
    /// A numbering style (`w:type="numbering"`).
    Numbering,
}

/// A single style definition from `w:style` (ECMA-376 §17.7.4.17).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DocxStyle {
    /// `@w:type` — paragraph, character, table, or numbering.
    pub style_type: DocxStyleType,
    /// `@w:styleId` — the unique identifier used in `w:pStyle` / `w:rStyle`.
    pub style_id: String,
    /// `@w:default="1"` — this is the default style for its type.
    pub is_default: bool,
    /// `@w:customStyle="1"` — this is a user-defined style.
    pub is_custom: bool,
    /// `w:name @w:val` — the display name.
    pub name: Option<String>,
    /// `w:basedOn @w:val` — parent style id.
    pub based_on: Option<String>,
    /// `w:next @w:val` — next paragraph style id.
    pub next: Option<String>,
    /// `w:link @w:val` — linked style id (char↔para).
    pub link: Option<String>,
    /// Paragraph properties.
    pub ppr: Option<DocxPPr>,
    /// Run (character) properties.
    pub rpr: Option<DocxRPr>,
}

/// Intermediate model for a table (`w:tbl`).
/// Placeholder for table parsing — used in `DocxBodyChild`.
/// ECMA-376 §17.4.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct DocxTableModel {
    /// Table properties from `w:tblPr`.
    pub tbl_pr: Option<DocxTblPr>,
    /// Column grid from `w:tblGrid`.
    pub col_widths: Vec<i32>,
    /// Rows.
    pub rows: Vec<DocxTableRow>,
}

/// Table properties from `w:tblPr` (ECMA-376 §17.4.60).
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct DocxTblPr {
    /// Style id from `w:tblStyle @w:val`.
    pub style_id: Option<String>,
    /// Table width from `w:tblW`.
    pub width: Option<DocxTblWidth>,
    /// `w:tblLayout @w:type` — `"fixed"` or `"autofit"` (the default).
    pub layout: Option<String>,
}

/// Table width specification from `w:tblW` (ECMA-376 §17.4.63).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DocxTblWidth {
    /// `@w:w` — width value.
    pub w: i32,
    /// `@w:type` — unit type (`"dxa"`, `"pct"`, `"auto"`, `"nil"`).
    pub w_type: String,
}

/// A table row from `w:tr` (ECMA-376 §17.4.79).
#[derive(Debug, Clone, Default)]
pub struct DocxTableRow {
    /// Row properties from `w:trPr`.
    pub tr_pr: Option<DocxTrPr>,
    /// Cells.
    pub cells: Vec<DocxTableCell>,
}

/// Table row properties from `w:trPr` (ECMA-376 §17.4.82).
#[derive(Debug, Clone, Default)]
pub struct DocxTrPr {
    /// `w:tblHeader` — this row is a header row.
    pub is_header: bool,
}

/// A table cell from `w:tc` (ECMA-376 §17.4.4).
#[derive(Debug, Clone, Default)]
pub struct DocxTableCell {
    /// Cell properties from `w:tcPr`.
    pub tc_pr: Option<DocxTcPr>,
    /// Content paragraphs and nested tables.
    pub paragraphs: Vec<super::paragraph::DocxParagraph>,
}

/// Cell margins from `w:tcMar` (ECMA-376 §17.4.68).
/// All values are in twentieths of a point (twips).
#[derive(Debug, Clone, Default)]
pub struct DocxCellMargins {
    pub top: Option<i32>,
    pub bottom: Option<i32>,
    pub left: Option<i32>,
    pub right: Option<i32>,
}

/// Vertical alignment from `w:vAlign @w:val` (ECMA-376 §17.4.84).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocxVAlign {
    Top,
    Center,
    Bottom,
}

/// Text direction from `w:textDirection @w:val` (ECMA-376 §17.4.87).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocxTextDirection {
    LrTb,
    TbRl,
    TbLr,
    BtLr,
}

/// Table cell properties from `w:tcPr` (ECMA-376 §17.4.70).
#[derive(Debug, Clone, Default)]
pub struct DocxTcPr {
    /// Column span from `w:gridSpan @w:val`.
    pub grid_span: Option<u32>,
    /// Vertical merge from `w:vMerge`.
    pub v_merge: Option<DocxVMerge>,
    /// Cell shading fill color from `w:shd @w:fill` (hex, no `#`).
    pub shd_fill: Option<String>,
    /// Cell shading pattern from `w:shd @w:val` (e.g. `clear`, `pct25`).
    pub shd_val: Option<String>,
    /// Cell shading pattern foreground from `w:shd @w:color` (hex).
    pub shd_color: Option<String>,
    /// Cell borders from `w:tcBorders`.
    pub tc_borders: Option<DocxTcBorders>,
    /// Cell margins from `w:tcMar`. Values in twips; divide by 20 for points.
    pub tc_margins: Option<DocxCellMargins>,
    /// Vertical alignment from `w:vAlign`.
    pub v_align: Option<DocxVAlign>,
    /// Text direction from `w:textDirection`.
    pub text_direction: Option<DocxTextDirection>,
}

/// Table cell borders from `w:tcBorders` (ECMA-376 §17.4.67).
#[derive(Debug, Clone, Default)]
pub struct DocxTcBorders {
    pub top: Option<DocxBorderEdge>,
    pub bottom: Option<DocxBorderEdge>,
    pub left: Option<DocxBorderEdge>,
    pub right: Option<DocxBorderEdge>,
}

/// Vertical merge information from `w:vMerge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocxVMerge {
    /// `w:vMerge @w:val="restart"` — start of a merged region.
    Restart,
    /// `w:vMerge` with no val — continuation of a merged region.
    Continue,
}
