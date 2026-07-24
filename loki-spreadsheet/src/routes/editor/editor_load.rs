// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document loading pipeline for spreadsheet.

use loki_doc_model::io::macros::MacroPayload;
use loki_file_access::FileAccessToken;
use loki_ooxml::xlsx::import::{XlsxImport, XlsxImportOptions};
use loki_sheet_model::Workbook;

use super::formula::UdfResolver;
use crate::error::LoadError;
use crate::new_document;

/// Builds the compute-only UDF resolver from a loaded document's macro payload
/// (macro spec §6.3), or `None` when it carries no readable procedures.
pub(super) fn udf_from(loaded: &LoadedDoc) -> Option<UdfResolver> {
    loaded.macros.as_ref().and_then(UdfResolver::from_payload)
}

/// A loaded spreadsheet plus any preserved macro payload (for compute-only UDFs,
/// macro spec §6.3). The payload is `None` for macro-free or untitled documents.
pub(super) struct LoadedDoc {
    /// The imported workbook.
    pub workbook: Workbook,
    /// The preserved VBA/Basic macro payload, if the file carried one.
    pub macros: Option<MacroPayload>,
}

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

/// Deserialise `path` → detect format → open file → import → return the
/// workbook and any preserved macro payload.
pub(super) fn load_document(path: String) -> Result<LoadedDoc, LoadError> {
    if new_document::is_untitled(&path) {
        return Ok(LoadedDoc {
            workbook: Workbook::new(),
            macros: None,
        });
    }
    let token = FileAccessToken::deserialize(&path)?;
    let format = detect_format(&token);
    let reader = token.open_read()?;
    let loaded = match format {
        DocumentFormat::Xlsx => {
            let r =
                XlsxImport::run(reader, XlsxImportOptions::default()).map_err(LoadError::Ooxml)?;
            LoadedDoc {
                workbook: r.workbook,
                macros: r.macros,
            }
        }
        DocumentFormat::Ods => {
            let r = loki_odf::OdsImport::run(reader, loki_odf::OdsImportOptions::default())
                .map_err(LoadError::Odf)?;
            LoadedDoc {
                workbook: r.workbook,
                macros: r.macros,
            }
        }
        DocumentFormat::Unsupported(ext) => {
            return Err(LoadError::UnsupportedFormat(ext));
        }
    };
    Ok(loaded)
}
