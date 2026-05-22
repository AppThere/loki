// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document loading pipeline for spreadsheet.

use loki_file_access::FileAccessToken;
use loki_ooxml::xlsx::import::{XlsxImport, XlsxImportOptions};
use loki_sheet_model::Workbook;

use crate::error::LoadError;
use crate::new_document;

/// Detected document format.
pub(super) enum DocumentFormat {
    Xlsx,
    Ods,
    Unsupported(String),
}

/// Inspect the display name on `token` and return the [`DocumentFormat`].
pub(super) fn detect_format(token: &FileAccessToken) -> DocumentFormat {
    match token
        .display_name()
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("xlsx") => DocumentFormat::Xlsx,
        Some("ods") => DocumentFormat::Ods,
        Some(ext) => DocumentFormat::Unsupported(ext.to_string()),
        None => DocumentFormat::Unsupported(String::new()),
    }
}

/// Deserialise `path` → detect format → open file → import → return [`Workbook`].
pub(super) fn load_document(path: String) -> Result<Workbook, LoadError> {
    if new_document::is_untitled(&path) {
        return Ok(Workbook::new());
    }
    let token = FileAccessToken::deserialize(&path)?;
    let format = detect_format(&token);
    let reader = token.open_read()?;
    let wb = match format {
        DocumentFormat::Xlsx => {
            XlsxImport::import(reader, XlsxImportOptions::default()).map_err(LoadError::Ooxml)?
        }
        DocumentFormat::Ods => {
            loki_odf::OdsImport::import(reader, loki_odf::OdsImportOptions::default())
                .map_err(LoadError::Odf)?
        }
        DocumentFormat::Unsupported(ext) => {
            return Err(LoadError::UnsupportedFormat(ext));
        }
    };
    Ok(wb)
}
