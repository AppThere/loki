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
