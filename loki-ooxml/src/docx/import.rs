// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX import entry point.
//!
//! [`DocxImport`] implements [`loki_doc_model::io::DocumentImport`] and is
//! the primary public API for converting a DOCX file into a
//! [`loki_doc_model::Document`].
//!
//! # Example
//!
//! ```no_run
//! use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
//! use loki_doc_model::io::DocumentImport;
//! let file = std::fs::File::open("document.docx").unwrap();
//! let doc = DocxImport::import(file, DocxImportOptions::default()).unwrap();
//! ```

use std::io::{Read, Seek};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_opc::Package;

use crate::error::{OoxmlError, OoxmlResult, OoxmlWarning};

#[path = "import_pic_bullets.rs"]
mod import_pic_bullets;
#[path = "import_package.rs"]
mod package;
pub(crate) use package::parse_and_map_package;

/// Options controlling DOCX import behaviour.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct DocxImportOptions {
    /// When `true` (default), paragraphs whose style name starts with
    /// `"heading"` map to a `Block::Heading` rather than a plain paragraph.
    pub emit_heading_blocks: bool,

    /// When `true` (default), images are embedded as data URIs
    /// (`data:<media-type>;base64,<data>`); when `false`, image parts are
    /// omitted from the output.
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

/// Unit struct that implements [`DocumentImport`] for DOCX files.
///
/// Construct import options with [`DocxImportOptions`] and call
/// [`DocumentImport::import`], or use [`DocxImporter`] directly for access
/// to the full [`DocxImportResult`] (including warnings).
pub struct DocxImport;

impl DocumentImport for DocxImport {
    type Error = OoxmlError;
    type Options = DocxImportOptions;

    /// Imports a DOCX file and returns the abstract document.
    ///
    /// Warnings are discarded. Use [`DocxImporter`] to retrieve them.
    fn import(reader: impl Read + Seek, options: Self::Options) -> Result<Document, Self::Error> {
        DocxImporter::new(options).run(reader).map(|r| r.document)
    }
}

/// Stateful DOCX importer that preserves [`OoxmlWarning`]s alongside the
/// imported [`Document`] — use it when you need the non-fatal import issues.
pub struct DocxImporter {
    options: DocxImportOptions,
}

impl DocxImporter {
    /// Creates a new importer with the given options.
    #[must_use]
    pub fn new(options: DocxImportOptions) -> Self {
        Self { options }
    }

    /// Opens the DOCX container and translates it into a [`DocxImportResult`]:
    /// open the OPC/ZIP package, locate the main `officeDocument` part, parse
    /// the body/styles/numbering/notes XML, collect hyperlink targets and
    /// (optionally) image bytes, then call `map_document`.
    ///
    /// # Errors
    ///
    /// Returns an error if the ZIP container is malformed, the required
    /// `officeDocument` relationship is missing, or a mandatory part fails to
    /// parse.
    pub fn run(self, reader: impl Read + Seek) -> OoxmlResult<DocxImportResult> {
        let package = Package::open(reader)?;
        let (document, warnings) = parse_and_map_package(&package, &self.options)?;
        Ok(DocxImportResult { document, warnings })
    }
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
