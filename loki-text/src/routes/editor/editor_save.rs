// SPDX-License-Identifier: Apache-2.0

//! Document save logic for the document editor.
//!
//! [`save_document_to_path`] serialises the live document from [`DocumentState`]
//! and writes it back to the file identified by the route `path` token.

use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_file_access::FileAccessToken;
use loki_ooxml::DocxExport;

use crate::editing::state::DocumentState;
use crate::new_document::is_untitled;

/// Errors that can occur when saving a document.
#[derive(Debug, thiserror::Error)]
pub(super) enum SaveError {
    /// No document is currently loaded in the editor.
    #[error("No document loaded")]
    NoDocument,
    /// The document serialiser returned an error.
    #[error("Export failed: {0}")]
    Export(String),
    /// An I/O error occurred writing to the file token.
    #[error("I/O error: {0}")]
    Io(String),
    /// The route path could not be parsed as a [`FileAccessToken`].
    #[error("Invalid token: {0}")]
    InvalidToken(String),
    /// The file format is not supported for saving (e.g. ODT, or untitled).
    #[error("Cannot save: {0}")]
    UnsupportedFormat(String),
}

/// Saves the current document to the file identified by `path`.
///
/// - Returns [`SaveError::UnsupportedFormat`] for untitled documents (no file
///   has been chosen yet via Save As) and for ODT files (export is a stub).
/// - Buffers the DOCX bytes in memory, then writes them to the file token in
///   one shot to avoid partial-write corruption.
pub(super) fn save_document_to_path(
    path: &str,
    doc_state: &Arc<Mutex<DocumentState>>,
) -> Result<(), SaveError> {
    if is_untitled(path) {
        return Err(SaveError::UnsupportedFormat(
            "untitled document — use File \u{2192} Save As".to_string(),
        ));
    }

    let token =
        FileAccessToken::deserialize(path).map_err(|e| SaveError::InvalidToken(e.to_string()))?;

    export_document_to_token(&token, doc_state)
}

/// Exports the current document to `token` as DOCX.
///
/// Shared by [`save_document_to_path`] (titled save) and the Save As flow,
/// which passes a freshly-picked destination token directly. Rejects ODT and
/// unknown formats; buffers the bytes in memory before a single write to avoid
/// partial-write corruption.
pub(super) fn export_document_to_token(
    token: &FileAccessToken,
    doc_state: &Arc<Mutex<DocumentState>>,
) -> Result<(), SaveError> {
    use super::editor_load::{DocumentFormat, detect_format};

    match detect_format(token) {
        DocumentFormat::Docx => {}
        DocumentFormat::Odt => {
            return Err(SaveError::UnsupportedFormat(
                "ODT saving is not yet supported".to_string(),
            ));
        }
        DocumentFormat::Unsupported(ext) => {
            return Err(SaveError::UnsupportedFormat(format!(
                "unknown format: {ext}"
            )));
        }
    }

    let arc_doc: Arc<Document> = doc_state
        .lock()
        .map_err(|_| SaveError::NoDocument)?
        .document
        .clone()
        .ok_or(SaveError::NoDocument)?;

    let mut buf = Cursor::new(Vec::<u8>::new());
    DocxExport::export(&arc_doc, &mut buf, ()).map_err(|e| SaveError::Export(e.to_string()))?;

    let bytes = buf.into_inner();
    let mut writer = token
        .open_write()
        .map_err(|e| SaveError::Io(e.to_string()))?;
    writer
        .write_all(&bytes)
        .map_err(|e| SaveError::Io(e.to_string()))?;

    Ok(())
}
