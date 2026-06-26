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
}

/// `w:cols` multi-column section layout (ECMA-376 §17.6.4).
#[derive(Debug, Clone)]
pub struct DocxCols {
    /// `@w:num` — the number of equal-width columns.
    pub num: i32,
    /// `@w:space` — the spacing between columns, in twips.
    pub space: i32,
    /// `@w:sep` — whether a separator line is drawn between columns.
    pub sep: bool,
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
