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
    /// Table-style formatting (only for `w:type="table"` styles): band sizes,
    /// base cell shading, and `w:tblStylePr` conditional regions.
    pub table: Option<DocxTableStyleProps>,
}

/// Table-style formatting parsed from a `w:type="table"` style: band sizes
/// (`w:tblStyleRowBandSize`/`w:tblStyleColBandSize`), the base whole-table
/// cell shading (`w:tcPr/w:shd`), and each `w:tblStylePr` region's shading.
/// ECMA-376 §17.7.6.
#[derive(Debug, Clone, Default)]
pub struct DocxTableStyleProps {
    /// `w:tblStyleRowBandSize @w:val` — rows per horizontal band.
    pub row_band_size: Option<u32>,
    /// `w:tblStyleColBandSize @w:val` — columns per vertical band.
    pub col_band_size: Option<u32>,
    /// Base whole-table cell shading fill from `w:tcPr/w:shd @w:fill`.
    pub base_shd_fill: Option<String>,
    /// Whole-table borders from `w:tblPr/w:tblBorders` — the outer edges plus
    /// the interior gridlines a *Table Grid* style draws.
    pub tbl_borders: Option<DocxTblBorders>,
    /// Per-region conditional formats from `w:tblStylePr`.
    pub conditional: Vec<DocxTblStylePr>,
}

/// Table borders from `w:tblBorders` (ECMA-376 §17.4.39): the four outer edges
/// plus the interior horizontal/vertical gridlines.
#[derive(Debug, Clone, Default)]
pub struct DocxTblBorders {
    pub top: Option<DocxBorderEdge>,
    pub bottom: Option<DocxBorderEdge>,
    pub left: Option<DocxBorderEdge>,
    pub right: Option<DocxBorderEdge>,
    pub inside_h: Option<DocxBorderEdge>,
    pub inside_v: Option<DocxBorderEdge>,
}

/// One `w:tblStylePr` conditional format (ECMA-376 §17.7.6.6). Only cell
/// shading is captured today; borders and run/paragraph props are future work.
#[derive(Debug, Clone, Default)]
pub struct DocxTblStylePr {
    /// `@w:type` — the region: `firstRow`, `lastRow`, `band1Horz`, `nwCell`, …
    pub region: String,
    /// Cell shading fill from this region's `w:tcPr/w:shd @w:fill`.
    pub shd_fill: Option<String>,
    /// Character formatting from this region's `w:rPr` (4a.3 — e.g. the
    /// bold header row of a built-in banded style).
    pub rpr: Option<super::paragraph::DocxRPr>,
}

/// Intermediate model for a table (`w:tbl`).
/// Placeholder for table parsing — used in `DocxBodyChild`.
/// ECMA-376 §17.4.
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
#[derive(Debug, Clone, Default)]
pub struct DocxTblPr {
    /// Style id from `w:tblStyle @w:val`.
    pub style_id: Option<String>,
    /// Table width from `w:tblW`.
    pub width: Option<DocxTblWidth>,
    /// `w:tblLayout @w:type` — `"fixed"` or `"autofit"` (the default).
    pub layout: Option<String>,
    /// `w:tblLook` region flags — which conditional style regions apply.
    pub tbl_look: Option<DocxTblLook>,
}

/// `w:tblLook` region flags (ECMA-376 §17.4.56) selecting which conditional
/// style regions apply to a table instance. Parsed from the explicit
/// attributes (`w:firstRow`, …) or the legacy `w:val` bitmask.
// The six flags mirror the OOXML `w:tblLook` bit fields one-for-one — a
// struct of bools is the faithful representation.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DocxTblLook {
    /// Apply the header-row (`firstRow`) conditional format.
    pub first_row: bool,
    /// Apply the total-row (`lastRow`) conditional format.
    pub last_row: bool,
    /// Apply the first-column conditional format.
    pub first_column: bool,
    /// Apply the last-column conditional format.
    pub last_column: bool,
    /// Apply row (horizontal) banding — `true` when `noHBand` is off.
    pub h_band: bool,
    /// Apply column (vertical) banding — `true` when `noVBand` is off.
    pub v_band: bool,
}

/// Table width specification from `w:tblW` (ECMA-376 §17.4.63).
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
    /// Ordered block-level content: paragraphs and nested tables (`w:tbl`
    /// inside `w:tc`, ECMA-376 §17.4.4). Reuses the body child enum so a cell
    /// can interleave paragraphs and tables in document order.
    pub children: Vec<super::document::DocxBodyChild>,
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
    /// Explicit conditional-format mask from `w:cnfStyle @w:val` (§17.4.7).
    pub cnf_style: Option<String>,
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
