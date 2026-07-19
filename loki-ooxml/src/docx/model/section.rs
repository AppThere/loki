// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Section and page-layout intermediate model structs.
//!
//! Split from `paragraph.rs` to keep individual files under the 300-line ceiling.
//! ECMA-376 §17.6 (sections) and §17.6.11–17.6.13 (page geometry).

/// Section properties from `w:sectPr` (ECMA-376 §17.6.17).
#[derive(Debug, Clone, Default)]
pub struct DocxSectPr {
    /// Page size from `w:pgSz`.
    pub pg_sz: Option<DocxPgSz>,
    /// Page margins from `w:pgMar`.
    pub pg_mar: Option<DocxPgMar>,
    /// Header references (type → `rel_id`).
    pub header_refs: Vec<DocxHdrFtrRef>,
    /// Footer references.
    pub footer_refs: Vec<DocxHdrFtrRef>,
    /// `<w:titlePg/>` — distinct first-page header/footer active (ECMA-376 §17.6.17).
    pub title_page: bool,
    /// Multi-column layout from `w:cols` (ECMA-376 §17.6.4).
    pub cols: Option<DocxCols>,
    /// `w:pgNumType @w:fmt` — page-number format (e.g. `lowerRoman`,
    /// `upperRoman`, `lowerLetter`). `None` = decimal (ECMA-376 §17.6.12).
    pub pg_num_fmt: Option<String>,
    /// `w:pgNumType @w:start` — page-number restart value for the section.
    pub pg_num_start: Option<u32>,
    /// `w:type @w:val` — how the section begins relative to the previous one
    /// (`continuous`, `nextPage`, `evenPage`, `oddPage`). `None` = `nextPage`
    /// (ECMA-376 §17.6.22).
    pub section_type: Option<String>,
    /// Page borders from `w:pgBorders` (ECMA-376 §17.6.10).
    pub pg_borders: Option<DocxPgBorders>,
    /// Margin line numbering from `w:lnNumType` (ECMA-376 §17.6.8).
    pub ln_num_type: Option<DocxLnNumType>,
}

/// `w:lnNumType` — margin line numbering (ECMA-376 §17.6.8). Attribute values
/// are raw; the mapper applies defaults and unit conversion.
#[derive(Debug, Clone, Default)]
pub struct DocxLnNumType {
    /// `@w:countBy` — print a number every N lines. `None` = every line.
    pub count_by: Option<u32>,
    /// `@w:start` — the first line number. `None` = `1`.
    pub start: Option<i32>,
    /// `@w:restart` — `newPage` (default) / `newSection` / `continuous`.
    pub restart: Option<String>,
    /// `@w:distance` — gutter between numbers and text, in twips. `None` = auto.
    pub distance: Option<i32>,
}

/// `w:pgBorders` — decorative border drawn around each page (ECMA-376 §17.6.10).
#[derive(Debug, Clone, Default)]
pub struct DocxPgBorders {
    pub top: Option<super::paragraph::DocxBorderEdge>,
    pub bottom: Option<super::paragraph::DocxBorderEdge>,
    pub left: Option<super::paragraph::DocxBorderEdge>,
    pub right: Option<super::paragraph::DocxBorderEdge>,
    /// `@w:offsetFrom` — `"text"` insets each edge from the text area; the
    /// default `"page"` (or absent) insets from the physical page edge.
    pub offset_from_text: bool,
}

/// `w:cols` multi-column section layout (ECMA-376 §17.6.4).
#[derive(Debug, Clone)]
pub struct DocxCols {
    /// `@w:num` — the number of columns.
    pub num: i32,
    /// `@w:space` — the spacing between columns, in twips.
    pub space: i32,
    /// `@w:sep` — whether a separator line is drawn between columns.
    pub sep: bool,
    /// Per-column widths in twips from `w:col @w:w` children, present only when
    /// `@w:equalWidth="0"`. Empty = equal-width columns.
    pub col_widths: Vec<i32>,
}

/// `w:pgSz` page size (ECMA-376 §17.6.13).
#[derive(Debug, Clone)]
pub struct DocxPgSz {
    /// `@w:w` — page width in twips.
    pub w: i32,
    /// `@w:h` — page height in twips.
    pub h: i32,
    /// `@w:orient` — orientation (`"landscape"` or `"portrait"`).
    pub orient: Option<String>,
}

/// `w:pgMar` page margins (ECMA-376 §17.6.11).
#[derive(Debug, Clone)]
pub struct DocxPgMar {
    /// `@w:top` in twips.
    pub top: i32,
    /// `@w:bottom` in twips.
    pub bottom: i32,
    /// `@w:left` in twips.
    pub left: i32,
    /// `@w:right` in twips.
    pub right: i32,
    /// `@w:header` in twips.
    pub header: i32,
    /// `@w:footer` in twips.
    pub footer: i32,
    /// `@w:gutter` in twips.
    pub gutter: i32,
}

/// A header or footer reference from `w:headerReference` / `w:footerReference`.
/// ECMA-376 §17.10.5 / §17.10.3.
#[derive(Debug, Clone)]
pub struct DocxHdrFtrRef {
    /// `@w:type` — `"default"`, `"first"`, or `"even"`.
    pub hf_type: String,
    /// `@r:id` — relationship id.
    pub rel_id: String,
}
