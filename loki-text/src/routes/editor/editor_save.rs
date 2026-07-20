// SPDX-License-Identifier: Apache-2.0

//! Document save logic for the document editor.
//!
//! [`save_document_to_path`] serialises the live document from [`DocumentState`]
//! and writes it back to the file identified by the route `path` token.

use std::io::{Cursor, Read, Write};
use std::sync::{Arc, Mutex};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_file_access::FileAccessToken;
use loki_odf::odt::export::{OdtExport, OdtExportOptions};
use loki_ooxml::{DocxExport, DocxTemplateExport};

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

/// Exports the current document to `token`, choosing DOCX or ODT by the
/// destination's extension.
///
/// Shared by [`save_document_to_path`] (titled save) and the Save As flow,
/// which passes a freshly-picked destination token directly. Rejects unknown
/// formats; buffers the bytes in memory before a single write to avoid
/// partial-write corruption.
pub(super) fn export_document_to_token(
    token: &FileAccessToken,
    doc_state: &Arc<Mutex<DocumentState>>,
) -> Result<(), SaveError> {
    use super::editor_load::{DocumentFormat, detect_format};

    let arc_doc = current_document(doc_state)?;
    let mut buf = Cursor::new(Vec::<u8>::new());
    match detect_format(token) {
        DocumentFormat::Docx => {
            DocxExport::export(&arc_doc, &mut buf, ())
                .map_err(|e| SaveError::Export(e.to_string()))?;
        }
        DocumentFormat::Odt => {
            OdtExport::export(&arc_doc, &mut buf, OdtExportOptions::default())
                .map_err(|e| SaveError::Export(e.to_string()))?;
        }
        DocumentFormat::Unsupported(ext) => {
            return Err(SaveError::UnsupportedFormat(format!(
                "unknown format: {ext}"
            )));
        }
    }
    write_all_to_token(token, &buf.into_inner())
}

/// Exports the current document to `token` as a Word **template** (`.dotx`).
///
/// Used by the "Save as Template" flow. Identical to [`export_document_to_token`]
/// except it writes the template content type, so Office treats the saved file
/// as a template (new documents are created from it) rather than editing it in
/// place. Rejects ODT (no OTT export yet) and unknown formats.
pub(super) fn export_template_to_token(
    token: &FileAccessToken,
    doc_state: &Arc<Mutex<DocumentState>>,
) -> Result<(), SaveError> {
    use super::editor_load::{DocumentFormat, detect_format};

    match detect_format(token) {
        DocumentFormat::Docx => {}
        DocumentFormat::Odt => {
            return Err(SaveError::UnsupportedFormat(
                "OTT template export is not yet supported".to_string(),
            ));
        }
        DocumentFormat::Unsupported(ext) => {
            return Err(SaveError::UnsupportedFormat(format!(
                "unknown format: {ext}"
            )));
        }
    }

    let arc_doc = current_document(doc_state)?;
    let mut buf = Cursor::new(Vec::<u8>::new());
    DocxTemplateExport::export(&arc_doc, &mut buf, ())
        .map_err(|e| SaveError::Export(e.to_string()))?;
    write_all_to_token(token, &buf.into_inner())
}

/// Repairs the on-disk `.docx` at `path` **losslessly**: reorders the OOXML
/// child elements into the schema sequence Microsoft Word requires, touching
/// nothing else (attributes, text, and constructs Loki does not model survive
/// verbatim). Returns the number of problems fixed (0 = already clean).
///
/// This deliberately operates on the file bytes, not the in-memory document —
/// Loki's tolerant reader already imported a correct model, so a model
/// round-trip would only risk dropping what Loki cannot represent. Reuses the
/// same single-write path as save to avoid partial-write corruption.
pub(super) fn repair_document_file(path: &str) -> Result<usize, SaveError> {
    use super::editor_load::{DocumentFormat, detect_format};

    if is_untitled(path) {
        return Err(SaveError::UnsupportedFormat(
            "untitled document — save it first".to_string(),
        ));
    }
    let token =
        FileAccessToken::deserialize(path).map_err(|e| SaveError::InvalidToken(e.to_string()))?;
    if !matches!(detect_format(&token), DocumentFormat::Docx) {
        return Err(SaveError::UnsupportedFormat(
            "repair currently supports .docx only".to_string(),
        ));
    }

    let mut reader = token
        .open_read()
        .map_err(|e| SaveError::Io(e.to_string()))?;
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| SaveError::Io(e.to_string()))?;

    let (fixed, report) =
        loki_ooxml::repair_docx(&bytes).map_err(|e| SaveError::Export(e.to_string()))?;
    if report.is_clean() {
        return Ok(0);
    }
    write_all_to_token(&token, &fixed)?;
    Ok(report.findings.len())
}

/// Clones the currently-loaded document out of `doc_state`.
fn current_document(doc_state: &Arc<Mutex<DocumentState>>) -> Result<Arc<Document>, SaveError> {
    doc_state
        .lock()
        .map_err(|_| SaveError::NoDocument)?
        .document
        .clone()
        .ok_or(SaveError::NoDocument)
}

/// Writes `bytes` to `token` in a single call (buffered upstream to avoid
/// partial-write corruption).
fn write_all_to_token(token: &FileAccessToken, bytes: &[u8]) -> Result<(), SaveError> {
    let mut writer = token
        .open_write()
        .map_err(|e| SaveError::Io(e.to_string()))?;
    writer
        .write_all(bytes)
        .map_err(|e| SaveError::Io(e.to_string()))?;
    Ok(())
}
