// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! DOCX import entry point.
//!
//! [`DocxImport`] implements [`loki_doc_model::io::DocumentImport`] and is
//! the primary public API for converting a DOCX file into a
//! [`loki_doc_model::Document`].
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};
//! use loki_doc_model::io::DocumentImport;
//!
//! let file = File::open("document.docx").unwrap();
//! let doc = DocxImport::import(file, DocxImportOptions::default()).unwrap();
//! ```

use std::io::{Read, Seek};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_opc::Package;

use crate::constants::REL_OFFICE_DOCUMENT;
use crate::error::{OoxmlError, OoxmlResult, OoxmlWarning};

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
    fn import(
        reader: impl Read + Seek,
        options: Self::Options,
    ) -> Result<Document, Self::Error> {
        DocxImporter::new(options).run(reader).map(|r| r.document)
    }
}

/// Stateful DOCX importer that preserves [`OoxmlWarning`]s alongside the
/// imported [`Document`].
///
/// Use this type when you need to inspect non-fatal import issues.
pub struct DocxImporter {
    options: DocxImportOptions,
}

impl DocxImporter {
    /// Creates a new importer with the given options.
    #[must_use]
    pub fn new(options: DocxImportOptions) -> Self {
        Self { options }
    }

    /// Opens the DOCX container, validates that it contains a main document
    /// part, and returns a [`DocxImportResult`].
    ///
    /// # Errors
    ///
    /// Returns an error if the ZIP container is malformed, if the required
    /// `officeDocument` relationship is missing, or if any part cannot be
    /// parsed.
    pub fn run(self, reader: impl Read + Seek) -> OoxmlResult<DocxImportResult> {
        let package = Package::open(reader)?;

        // Locate the main document part via the package-level relationship.
        let _doc_rel = package
            .relationships()
            .by_type(REL_OFFICE_DOCUMENT)
            .next()
            .ok_or_else(|| OoxmlError::MissingPart {
                relationship_type: REL_OFFICE_DOCUMENT.to_owned(),
            })?;

        // The full mapper pipeline (XML parse → intermediate model → loki_doc_model)
        // will be wired here in v0.2.0. For now return a well-formed empty document
        // so that the crate compiles and tests can exercise the import entry point.
        let _ = &self.options; // suppress unused-field warning until mapper is wired

        Ok(DocxImportResult {
            document: Document::new(),
            warnings: Vec::new(),
        })
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
