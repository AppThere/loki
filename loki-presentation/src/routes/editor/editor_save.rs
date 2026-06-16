// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Save pipeline for the presentation editor: serialise the model to a PPTX
//! package and write it to a file token.

use std::io::{Cursor, Write};

use loki_file_access::FileAccessToken;
use loki_ooxml::pptx::export::PptxExport;
use loki_presentation_model::Presentation;

use super::editor_load::{DocumentFormat, detect_format};

/// Exports `pres` to `token` as a PPTX package.
///
/// Buffers the bytes in memory before a single write to avoid partial-write
/// corruption. Returns a human-readable error string on failure. ODP export is
/// not yet implemented.
pub(super) fn export_to_token(token: &FileAccessToken, pres: &Presentation) -> Result<(), String> {
    match detect_format(token) {
        DocumentFormat::Pptx => {}
        DocumentFormat::Odp => return Err("ODP saving is not yet supported".to_string()),
        DocumentFormat::Unsupported(ext) => return Err(format!("unsupported format: .{ext}")),
    }

    let mut buf = Cursor::new(Vec::<u8>::new());
    PptxExport::export(pres, &mut buf).map_err(|e| e.to_string())?;
    let bytes = buf.into_inner();

    let mut writer = token.open_write().map_err(|e| e.to_string())?;
    writer.write_all(&bytes).map_err(|e| e.to_string())?;
    Ok(())
}
