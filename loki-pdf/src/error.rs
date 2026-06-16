// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Error type for PDF / PDF-X export.

/// Errors that can occur while exporting a [`loki_doc_model::Document`] to PDF.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PdfError {
    /// An I/O error occurred while writing the PDF bytes.
    #[error("PDF I/O error: {0}")]
    Io(String),

    /// A font referenced by the layout could not be parsed for embedding.
    ///
    /// PDF/X mandates that every font be embedded, so a parse failure is fatal
    /// rather than silently substituted.
    #[error("PDF font embedding error: {0}")]
    Font(String),

    /// The document produced no layoutable pages.
    #[error("PDF export error: document has no pages")]
    NoPages,
}

impl From<std::io::Error> for PdfError {
    fn from(e: std::io::Error) -> Self {
        PdfError::Io(e.to_string())
    }
}
