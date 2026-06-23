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
        // `.dotx` / `.dotm` are Word *templates*: structurally DOCX (same
        // `officeDocument` relationship), so the DOCX importer reads them as-is.
        Some("docx" | "dotx" | "dotm") => DocumentFormat::Docx,
        // `.ott` is a LibreOffice text *template*: structurally ODT (only the
        // package `mimetype` differs, which the importer now accepts).
        Some("odt" | "ott") => DocumentFormat::Odt,
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
    use loki_app_shell::NewDocSource;

    // Untitled paths encode how to build their initial content (blank, a bundled
    // template, or an imported external file) — see `loki_app_shell::untitled`.
    match new_document::parse_new_doc_source(&path) {
        Some(NewDocSource::Blank) => return Ok(Document::new_blank()),
        Some(NewDocSource::Template(id)) => return build_template(&id),
        Some(NewDocSource::Import(token)) => return import_token(&token),
        None => {} // real file path — fall through
    }
    import_token(&path)
}

/// Deserialises `serialized` as a file token, detects its format, and imports it.
/// Shared by the real-file open path and the "open external template as a fresh
/// document" path (both ultimately read a file token).
fn import_token(serialized: &str) -> Result<Document, LoadError> {
    // Open-path timing: file read + format import, logged under `loki_text::open`
    // so the read/import portion of open latency is measurable on-device. The
    // dominant open cost is the layout pass that follows (see `state::seed_*`).
    let started = std::time::Instant::now();
    let token = FileAccessToken::deserialize(serialized)?;
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
    tracing::info!(
        target: "loki_text::open",
        elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
        "load_document: file read + import complete",
    );
    Ok(doc)
}

/// Builds a bundled template document from its short `id` (see `loki-templates`).
///
/// An unknown id degrades to a blank document so a stale path never fails to
/// open a tab.
fn build_template(id: &str) -> Result<Document, LoadError> {
    Ok(loki_templates::document(id).unwrap_or_else(Document::new_blank))
}
