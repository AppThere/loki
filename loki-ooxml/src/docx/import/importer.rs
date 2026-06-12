// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`DocxImport`] and [`DocxImporter`] — public entry points for DOCX import.

use std::io::{Read, Seek};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_opc::Package;

use crate::error::{OoxmlError, OoxmlResult};

use super::options::{DocxImportOptions, DocxImportResult};
use super::pipeline::parse_and_map_package;

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

/// Stateful DOCX importer that preserves [`crate::error::OoxmlWarning`]s alongside the
/// imported [`loki_doc_model::Document`].
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

    /// Opens the DOCX container and translates it into a [`DocxImportResult`].
    ///
    /// Steps:
    /// 1. Open the OPC/ZIP package.
    /// 2. Locate the main `officeDocument` part via package relationships.
    /// 3. Parse XML for document body, styles, numbering, footnotes, endnotes.
    /// 4. Collect hyperlink targets and (optionally) image bytes.
    /// 5. Call `map_document` to produce the abstract model.
    ///
    /// # Errors
    ///
    /// Returns an error if the ZIP container is malformed, if the required
    /// `officeDocument` relationship is missing, or if any mandatory part
    /// cannot be parsed.
    pub fn run(self, reader: impl Read + Seek) -> OoxmlResult<DocxImportResult> {
        let package = Package::open(reader)?;
        let (document, warnings) = parse_and_map_package(&package, &self.options)?;
        Ok(DocxImportResult { document, warnings })
    }
}
