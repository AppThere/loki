// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! DOCX export stub.
//!
//! DOCX export is not implemented in `loki-ooxml` v0.1.0. Calling
//! [`DocxExport::export`] will always return
//! [`OoxmlError::ExportNotImplemented`].

use std::io::{Seek, Write};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;

use crate::error::OoxmlError;

/// Unit struct that implements [`DocumentExport`] for DOCX.
///
/// DOCX export is **not yet implemented**. Every call to
/// [`DocumentExport::export`] returns
/// [`OoxmlError::ExportNotImplemented`].
pub struct DocxExport;

impl DocumentExport for DocxExport {
    type Error = OoxmlError;
    type Options = ();

    /// Always returns [`OoxmlError::ExportNotImplemented`].
    fn export(
        _doc: &Document,
        _writer: impl Write + Seek,
        _options: Self::Options,
    ) -> Result<(), Self::Error> {
        Err(OoxmlError::ExportNotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn export_returns_not_implemented() {
        let doc = Document::new();
        let mut buf = Cursor::new(Vec::<u8>::new());
        let result = DocxExport::export(&doc, &mut buf, ());
        assert!(matches!(result, Err(OoxmlError::ExportNotImplemented)));
    }
}
