// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODS exporter.

mod content;
mod manifest;
mod xml_utils;

use loki_sheet_model::Workbook;
use std::io::{Seek, Write};
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use crate::constants::{ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_MIMETYPE, ENTRY_STYLES, MIME_ODS};
use crate::error::OdfError;

use self::content::generate_content;
use self::manifest::{generate_manifest, generate_styles};

/// Options controlling ODS export behaviour.
#[derive(Debug, Clone, Default)]
pub struct OdsExportOptions {}

/// Unit struct that implements ODS spreadsheet export.
pub struct OdsExport;

impl OdsExport {
    /// Export a [`Workbook`] to an ODS writer.
    pub fn export(workbook: &Workbook, writer: impl Write + Seek) -> Result<(), OdfError> {
        let mut zip = ZipWriter::new(writer);

        // 1. mimetype (stored, uncompressed)
        let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        zip.start_file(ENTRY_MIMETYPE, stored)?;
        zip.write_all(MIME_ODS.as_bytes())?;

        let deflated =
            FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

        // 2. META-INF/manifest.xml
        zip.start_file(ENTRY_MANIFEST, deflated)?;
        zip.write_all(generate_manifest().as_bytes())?;

        // 3. styles.xml
        zip.start_file(ENTRY_STYLES, deflated)?;
        zip.write_all(generate_styles().as_bytes())?;

        // 4. content.xml
        zip.start_file(ENTRY_CONTENT, deflated)?;
        zip.write_all(generate_content(workbook).as_bytes())?;

        zip.finish()?;

        Ok(())
    }
}
