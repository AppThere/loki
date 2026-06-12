// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared mapping context threaded through all content-level mappers.

use std::collections::HashMap;

use loki_doc_model::content::block::Block;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_opc::PartData;

use crate::docx::import::DocxImportOptions;
use crate::error::OoxmlWarning;

/// Shared state threaded through all content-level mappers.
pub(crate) struct MappingContext<'a> {
    /// The resolved style catalog for this document.
    pub styles: &'a StyleCatalog,
    /// Footnote content keyed by `w:footnote @w:id`.
    pub footnotes: &'a HashMap<i32, Vec<Block>>,
    /// Endnote content keyed by `w:endnote @w:id`.
    pub endnotes: &'a HashMap<i32, Vec<Block>>,
    /// External hyperlink targets: relationship id → URL.
    pub hyperlinks: &'a HashMap<String, String>,
    /// Image parts: relationship id → raw bytes + media type.
    pub images: &'a HashMap<String, PartData>,
    /// Import options controlling heading promotion, image embedding, etc.
    pub options: &'a DocxImportOptions,
    /// Non-fatal warnings accumulated during mapping.
    pub warnings: Vec<OoxmlWarning>,
    /// Stack of currently open bookmark IDs and their names, to resolve names at `BookmarkEnd`.
    pub open_bookmarks: Vec<(String, String)>,
}
