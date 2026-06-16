// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Error types for `loki-presentation`.

use thiserror::Error;

/// Errors that can occur while loading a presentation for the editor.
#[derive(Debug, Error)]
pub enum LoadError {
    /// The serialised token in the route path could not be decoded.
    #[error("could not parse file token: {0}")]
    TokenParse(#[from] loki_file_access::TokenParseError),

    /// The file could not be opened (permission revoked, I/O error, etc.).
    #[error("could not open file: {0}")]
    FileAccess(#[from] loki_file_access::AccessError),

    /// The PPTX import pipeline failed (malformed ZIP, missing parts, etc.).
    #[error("PPTX import failed: {0}")]
    Ooxml(#[from] loki_ooxml::OoxmlError),

    /// The file extension is not a supported presentation format.
    #[error("'.{0}' files are not yet supported")]
    UnsupportedFormat(String),
}
