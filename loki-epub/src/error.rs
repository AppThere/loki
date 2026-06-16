// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Error type for EPUB export.

/// Errors that can occur while exporting a [`loki_doc_model::Document`] to
/// EPUB 3.3.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EpubError {
    /// An I/O error occurred while writing the ZIP container.
    #[error("EPUB I/O error: {0}")]
    Io(String),

    /// The ZIP writer reported an error assembling the container.
    #[error("EPUB container error: {0}")]
    Zip(String),

    /// A required metadata field was missing and could not be synthesised.
    #[error("EPUB metadata error: {0}")]
    Metadata(String),
}

impl From<std::io::Error> for EpubError {
    fn from(e: std::io::Error) -> Self {
        EpubError::Io(e.to_string())
    }
}

impl From<zip::result::ZipError> for EpubError {
    fn from(e: zip::result::ZipError) -> Self {
        EpubError::Zip(e.to_string())
    }
}
