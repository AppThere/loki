// SPDX-License-Identifier: Apache-2.0

//! Document loading pipeline.
//!
//! Pipeline entry points (confirmed from source):
//! `loki_file_access`: `FileAccessToken::deserialize(s: &str)`
//!                     `token.open_read() -> Result<Box<dyn ReadSeek>, AccessError>`
//! `loki_ooxml`:       `DocxImport::import(reader, DocxImportOptions)`
//!                     (via `loki_doc_model::io::DocumentImport`)
//! `loki_odf`:         `OdtImport::import(reader, OdtImportOptions)`

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_file_access::FileAccessToken;
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};

use crate::error::LoadError;
use crate::new_document;

/// Detected document format, derived from the file extension in the token's
/// display name.
pub(super) enum DocumentFormat {
    Docx,
    Odt,
    Unsupported(String),
}

/// Inspect the display name on `token` and return the [`DocumentFormat`] for
/// this file.  The extension comparison is case-insensitive.
pub(super) fn detect_format(token: &FileAccessToken) -> DocumentFormat {
    match token
        .display_name()
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("docx") => DocumentFormat::Docx,
        Some("odt") => DocumentFormat::Odt,
        Some(ext) => DocumentFormat::Unsupported(ext.to_string()),
        None => DocumentFormat::Unsupported(String::new()),
    }
}

/// Deserialise `path` → detect format → open file → import → return [`Document`].
///
/// Format is determined from the file extension in the [`FileAccessToken`]
/// display name before the file is opened, so the reader is only consumed once.
/// All I/O is synchronous; called inside an `async move` block in
/// [`use_resource`] so loading does not block the initial render of the shell.
pub(super) fn load_document(path: String) -> Result<Document, LoadError> {
    if new_document::is_untitled(&path) {
        return Ok(Document::new_blank());
    }
    let token = FileAccessToken::deserialize(&path)?;
    let format = detect_format(&token);
    let reader = token.open_read()?;
    let doc = match format {
        DocumentFormat::Docx => {
            DocxImport::import(reader, DocxImportOptions::default()).map_err(LoadError::Ooxml)?
            // TODO(odt-fidelity): DOCX rendering gaps (styles, page size) tracked separately.
        }
        DocumentFormat::Odt => {
            OdtImport::import(reader, OdtImportOptions::default()).map_err(LoadError::Odt)?
            // TODO(odt-fidelity): ODT rendering gaps — paragraph styles, list indents,
            // and image placement may not render correctly yet.
        }
        DocumentFormat::Unsupported(ext) => {
            return Err(LoadError::UnsupportedFormat(ext));
        }
    };
    Ok(doc)
}
