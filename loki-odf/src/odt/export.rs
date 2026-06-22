// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT export.
//!
//! [`OdtExport`] writes a [`loki_doc_model::Document`] to an ODT
//! (`OpenDocument` Text) ZIP package: `mimetype`, `META-INF/manifest.xml`,
//! `content.xml`, `styles.xml`, and `meta.xml`. ODF 1.3 §3.

use std::io::{Seek, Write};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use crate::constants::{
    ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_META, ENTRY_MIMETYPE, ENTRY_STYLES, MIME_ODT,
};
use crate::error::{OdfError, OdfResult};
use crate::odt::write::{content_xml, meta_xml, styles_xml};

/// Options controlling ODT export behaviour.
///
/// Currently empty; reserved for future use (e.g. controlling whether images
/// are embedded or linked). ODF 1.3 §3.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct OdtExportOptions {}

/// Unit struct that implements [`DocumentExport`] for ODT files.
pub struct OdtExport;

impl DocumentExport for OdtExport {
    type Error = OdfError;
    type Options = OdtExportOptions;

    /// Exports a [`Document`] as an ODT package. ODF 1.3 §3.
    fn export(doc: &Document, writer: impl Write + Seek, _options: Self::Options) -> OdfResult<()> {
        let mut zip = ZipWriter::new(writer);

        // 1. mimetype — first entry, stored (uncompressed), no trailing newline.
        let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        zip.start_file(ENTRY_MIMETYPE, stored)?;
        zip.write_all(MIME_ODT.as_bytes())?;

        let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

        // 2. manifest, then the three XML parts.
        zip.start_file(ENTRY_MANIFEST, deflated)?;
        zip.write_all(MANIFEST.as_bytes())?;

        zip.start_file(ENTRY_CONTENT, deflated)?;
        zip.write_all(content_xml(doc).as_bytes())?;

        zip.start_file(ENTRY_STYLES, deflated)?;
        zip.write_all(styles_xml(doc).as_bytes())?;

        zip.start_file(ENTRY_META, deflated)?;
        zip.write_all(meta_xml(doc).as_bytes())?;

        zip.finish()?;
        Ok(())
    }
}

/// The ODT package manifest (fixed: the parts written are always the same).
const MANIFEST: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\"",
    " manifest:version=\"1.3\">",
    "<manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.3\"",
    " manifest:media-type=\"application/vnd.oasis.opendocument.text\"/>",
    "<manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/>",
    "<manifest:file-entry manifest:full-path=\"styles.xml\" manifest:media-type=\"text/xml\"/>",
    "<manifest:file-entry manifest:full-path=\"meta.xml\" manifest:media-type=\"text/xml\"/>",
    "</manifest:manifest>",
);
