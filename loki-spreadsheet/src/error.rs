// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Application-level error types for `loki-spreadsheet`.

use thiserror::Error;

/// Top-level application error.
#[derive(Debug, Error)]
pub enum AppError {
    /// The platform file picker failed to open or returned an error.
    #[error("file picker error: {0}")]
    FilePicker(#[from] loki_file_access::PickerError),

    /// A serialised [`loki_file_access::FileAccessToken`] could not be parsed.
    #[error("invalid file token: {0}")]
    TokenParse(#[from] loki_file_access::TokenParseError),
}

/// Errors that can occur during the document-loading pipeline in the editor.
#[derive(Debug, Error)]
pub enum LoadError {
    /// The serialised token in the route path could not be decoded.
    #[error("could not parse file token: {0}")]
    TokenParse(#[from] loki_file_access::TokenParseError),

    /// The file could not be opened (permission revoked, I/O error, etc.).
    #[error("could not open file: {0}")]
    FileAccess(#[from] loki_file_access::AccessError),

    /// The XLSX import pipeline failed (malformed ZIP, missing parts, etc.).
    #[error("XLSX import failed: {0}")]
    Ooxml(#[from] loki_ooxml::OoxmlError),

    /// The ODS import pipeline failed.
    #[error("ODS import failed: {0}")]
    Odf(#[from] loki_odf::OdfError),

    /// The file extension is not a supported document format.
    #[error("'.{0}' files are not yet supported")]
    UnsupportedFormat(String),
}
