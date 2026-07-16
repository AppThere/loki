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
use loki_doc_model::io::macros::MacroPayload;
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use crate::constants::{
    ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_META, ENTRY_MIMETYPE, ENTRY_STYLES, MIME_ODT,
};
use crate::error::{OdfError, OdfResult};
use crate::odt::write::{MathPart, MediaPart, content_xml, meta_xml, styles_xml};

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
        let content = content_xml(doc);
        let styles = styles_xml(doc);
        let scripts = crate::script_write::odf_script_payload(
            doc.source.as_ref().and_then(|s| s.macros.as_ref()),
        );

        let mut zip = ZipWriter::new(writer);

        // 1. mimetype — first entry, stored (uncompressed), no trailing newline.
        let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        zip.start_file(ENTRY_MIMETYPE, stored)?;
        zip.write_all(MIME_ODT.as_bytes())?;

        let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

        // 2. manifest (listing every part, including embedded images from both
        //    the body and the master-page header/footer, plus any embedded
        //    formula objects).
        zip.start_file(ENTRY_MANIFEST, deflated)?;
        zip.write_all(manifest(&content.media, &styles.media, &content.objects, scripts).as_bytes())?;

        // 3. the three XML parts.
        zip.start_file(ENTRY_CONTENT, deflated)?;
        zip.write_all(content.xml.as_bytes())?;

        zip.start_file(ENTRY_STYLES, deflated)?;
        zip.write_all(styles.xml.as_bytes())?;

        zip.start_file(ENTRY_META, deflated)?;
        zip.write_all(meta_xml(doc).as_bytes())?;

        // 4. embedded image parts (already-compressed images stay stored to
        //    avoid double-compression overhead).
        for part in content.media.iter().chain(styles.media.iter()) {
            zip.start_file(&part.path, stored)?;
            zip.write_all(&part.bytes)?;
        }

        // 5. embedded formula objects: each `Object N/content.xml` sub-document.
        for obj in &content.objects {
            zip.start_file(format!("{}/content.xml", obj.dir), deflated)?;
            zip.write_all(obj.content_xml.as_bytes())?;
        }

        // 6. preserved macro/script libraries (Basic/, Scripts/), verbatim.
        if let Some(payload) = scripts {
            crate::script_write::write_script_parts(&mut zip, payload)?;
        }

        zip.finish()?;
        Ok(())
    }
}

/// Builds `META-INF/manifest.xml`, listing the fixed parts, every image
/// (from the body and the master-page header/footer), and every embedded
/// formula object sub-document.
fn manifest(
    body_media: &[MediaPart],
    styles_media: &[MediaPart],
    objects: &[MathPart],
    scripts: Option<&MacroPayload>,
) -> String {
    let mut m = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\"",
        " manifest:version=\"1.3\">",
        "<manifest:file-entry manifest:full-path=\"/\" manifest:version=\"1.3\"",
        " manifest:media-type=\"application/vnd.oasis.opendocument.text\"/>",
        "<manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/>",
        "<manifest:file-entry manifest:full-path=\"styles.xml\" manifest:media-type=\"text/xml\"/>",
        "<manifest:file-entry manifest:full-path=\"meta.xml\" manifest:media-type=\"text/xml\"/>",
    ));
    for part in body_media.iter().chain(styles_media.iter()) {
        m.push_str(&format!(
            "<manifest:file-entry manifest:full-path=\"{}\" manifest:media-type=\"{}\"/>",
            part.path, part.media_type
        ));
    }
    // Each formula object contributes a directory entry (with the formula
    // media type) and its `content.xml` entry. ODF 1.3 §3.16.
    for obj in objects {
        m.push_str(&format!(
            "<manifest:file-entry manifest:full-path=\"{0}/\" \
             manifest:media-type=\"application/vnd.oasis.opendocument.formula\"/>\
             <manifest:file-entry manifest:full-path=\"{0}/content.xml\" \
             manifest:media-type=\"text/xml\"/>",
            obj.dir
        ));
    }
    // Preserved macro/script library entries (spec §3.3).
    if let Some(payload) = scripts {
        m.push_str(&crate::script_write::script_manifest_entries(payload));
    }
    m.push_str("</manifest:manifest>");
    m
}
