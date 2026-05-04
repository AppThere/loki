// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Intermediate model for `word/document.xml`.
//!
//! Mirrors ECMA-376 §17.2 (document structure) and §17.3 (block-level content).

use super::paragraph::{DocxParagraph, DocxSectPr};
use super::styles::DocxTableModel;

/// Top-level intermediate model for `w:document` (ECMA-376 §17.2.3).
#[derive(Debug, Clone, Default)]
pub struct DocxDocument {
    /// The document body.
    pub body: DocxBody,
}

/// Intermediate model for `w:body` (ECMA-376 §17.2.2).
#[derive(Debug, Clone, Default)]
pub struct DocxBody {
    /// Ordered block-level content children.
    pub children: Vec<DocxBodyChild>,
    /// Final section properties (`w:sectPr` as last child of `w:body`).
    /// ECMA-376 §17.6.17.
    pub final_sect_pr: Option<DocxSectPr>,
}

/// A block-level child of `w:body`.
// Enum is short-lived during parsing; boxing would add allocation overhead
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum DocxBodyChild {
    /// A paragraph (`w:p`). ECMA-376 §17.3.1.22.
    Paragraph(DocxParagraph),
    /// A table (`w:tbl`). ECMA-376 §17.4.
    Table(DocxTableModel),
    /// A structured document tag (`w:sdt`) — stored opaquely for now.
    Sdt,
}
