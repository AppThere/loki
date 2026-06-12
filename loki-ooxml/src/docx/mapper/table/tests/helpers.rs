// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared test helpers for table mapper tests.

use crate::docx::import::DocxImportOptions;
use crate::docx::mapper::document::MappingContext;
use crate::docx::model::paragraph::DocxParagraph;
use crate::docx::model::styles::{DocxTableCell, DocxTableRow};
use loki_doc_model::content::block::Block;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_opc::PartData;
use std::collections::HashMap;

pub(super) fn make_ctx<'a>(
    styles: &'a StyleCatalog,
    footnotes: &'a HashMap<i32, Vec<Block>>,
    endnotes: &'a HashMap<i32, Vec<Block>>,
    hyperlinks: &'a HashMap<String, String>,
    images: &'a HashMap<String, PartData>,
    options: &'a DocxImportOptions,
) -> MappingContext<'a> {
    MappingContext {
        styles,
        footnotes,
        endnotes,
        hyperlinks,
        images,
        options,
        warnings: Vec::new(),
        open_bookmarks: Vec::new(),
    }
}

pub(super) fn simple_cell(paragraphs: Vec<DocxParagraph>) -> DocxTableCell {
    DocxTableCell {
        tc_pr: None,
        paragraphs,
    }
}

pub(super) fn simple_row(cells: Vec<DocxTableCell>) -> DocxTableRow {
    DocxTableRow { tr_pr: None, cells }
}
