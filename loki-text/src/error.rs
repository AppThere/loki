// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Application-level error types for `loki-text`.
//!
//! [`AppError`] is the top-level error enum for UI-level failures (e.g. the
//! file picker).  [`LoadError`] covers the document-loading pipeline:
//! token parse → file open → format detection → OOXML or ODF import.

use thiserror::Error;

/// Top-level application error.
///
/// Variants wrap errors from the crates that `loki-text` depends on so that
/// they can be handled uniformly via the `?` operator and displayed in the UI.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum AppError {
    /// The platform file picker failed to open or returned an error.
    #[error("file picker error: {0}")]
    FilePicker(#[from] loki_file_access::PickerError),

    /// A serialised [`loki_file_access::FileAccessToken`] could not be parsed.
    #[error("invalid file token: {0}")]
    TokenParse(#[from] loki_file_access::TokenParseError),
}

/// Convenience `Result` alias using [`AppError`].
#[allow(dead_code)]
pub type AppResult<T> = Result<T, AppError>;

// ── LoadError ─────────────────────────────────────────────────────────────────

/// Errors that can occur during the document-loading pipeline in the editor.
///
/// The pipeline runs in a `use_resource` async block:
/// 1. Deserialise the route `path` into a [`loki_file_access::FileAccessToken`].
/// 2. Open the file for reading via [`loki_file_access::FileAccessToken::open_read`].
/// 3. Import the DOCX bytes into a [`loki_doc_model::Document`] via
///    [`loki_ooxml::DocxImport`].
///
/// Each step maps to exactly one variant here.
#[derive(Debug, Error)]
pub enum LoadError {
    /// The serialised token in the route path could not be decoded.
    #[error("could not parse file token: {0}")]
    TokenParse(#[from] loki_file_access::TokenParseError),

    /// The file could not be opened (permission revoked, I/O error, etc.).
    #[error("could not open file: {0}")]
    FileAccess(#[from] loki_file_access::AccessError),

    /// The DOCX import pipeline failed (malformed ZIP, missing parts, etc.).
    #[error("DOCX import failed: {0}")]
    Ooxml(loki_ooxml::OoxmlError),

    /// The ODT import pipeline failed (malformed ZIP, missing XML parts, etc.).
    #[error("ODT import failed: {0}")]
    Odt(loki_odf::OdfError),

    /// The file extension is not a supported document format.
    ///
    /// The inner string is the raw extension (without the leading dot) so that
    /// the UI can display "'.<ext>' files are not yet supported".
    #[error("'.{0}' files are not yet supported")]
    UnsupportedFormat(String),
}

/// Convenience `Result` alias using [`LoadError`].
#[allow(dead_code)]
pub type LoadResult<T> = Result<T, LoadError>;
