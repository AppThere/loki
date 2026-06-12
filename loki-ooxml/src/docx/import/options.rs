// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Import option types for DOCX import.

use loki_doc_model::document::Document;

use crate::error::OoxmlWarning;

/// Options controlling DOCX import behaviour.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct DocxImportOptions {
    /// When `true`, paragraphs whose style name starts with `"heading"` are
    /// mapped to [`loki_doc_model::content::block::Block::Heading`] rather
    /// than [`loki_doc_model::content::block::Block::Paragraph`].
    ///
    /// Defaults to `true`.
    pub emit_heading_blocks: bool,

    /// When `true`, images are embedded in the document as data URIs
    /// (`data:<media-type>;base64,<data>`). When `false`, image parts are
    /// omitted from the output.
    ///
    /// Defaults to `true`.
    pub embed_images: bool,
}

impl Default for DocxImportOptions {
    fn default() -> Self {
        Self {
            emit_heading_blocks: true,
            embed_images: true,
        }
    }
}

/// The result of a successful DOCX import.
#[derive(Debug)]
pub struct DocxImportResult {
    /// The imported document in the format-neutral abstract model.
    pub document: Document,

    /// Non-fatal issues encountered during import (unresolved relationships,
    /// unsupported features, etc.).
    pub warnings: Vec<OoxmlWarning>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_emit_heading_blocks() {
        assert!(DocxImportOptions::default().emit_heading_blocks);
    }

    #[test]
    fn default_options_embed_images() {
        assert!(DocxImportOptions::default().embed_images);
    }
}
