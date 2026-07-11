// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! EPUB 3.3 export for the Loki document suite.
//!
//! [`EpubExport`] turns a [`loki_doc_model::Document`] into a reflowable
//! EPUB 3.3 publication: an OCF (ZIP) container holding a package document,
//! a navigation document, one XHTML content document, and a stylesheet.
//!
//! The exporter implements [`loki_doc_model::io::DocumentExport`], so it plugs
//! into the same call site as the DOCX/ODT writers.
//!
//! # Conformance
//!
//! - The `mimetype` entry is written first and **stored** (uncompressed), as
//!   required by OCF §4.3.
//! - The package declares EPUB version 3.0 with the mandatory
//!   `dc:identifier` / `dc:title` / `dc:language` / `dcterms:modified`
//!   metadata (EPUB 3.3 §5.4).
//! - A single reflowable content document is emitted. Tables render as real
//!   `<table>`s (caption, `<colgroup>` widths, `colspan`/`rowspan`, resolved
//!   cell alignment); embedded `data:` images are decoded, packaged as manifest
//!   resources, and referenced with `<img>` (external URLs are referenced but
//!   not packaged). Fixed-layout and media overlays are out of scope.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod container;
pub mod content;
pub mod error;
pub mod images;
pub mod inlines;
pub mod nav;
pub mod opf_meta;
pub mod package;
pub mod tables;
mod xml;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{Seek, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use loki_doc_model::Document;
use loki_doc_model::io::DocumentExport;
use zip::write::SimpleFileOptions;

pub use error::EpubError;

/// Options controlling EPUB export. Reserved for future use (cover image,
/// per-chapter splitting); currently empty.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct EpubOptions {}

/// EPUB 3.3 exporter. Implements [`DocumentExport`].
pub struct EpubExport;

impl DocumentExport for EpubExport {
    type Error = EpubError;
    type Options = EpubOptions;

    fn export(
        doc: &Document,
        writer: impl Write + Seek,
        _options: Self::Options,
    ) -> Result<(), Self::Error> {
        write_epub(doc, writer)
    }
}

/// Writes the document to `writer` as an EPUB 3.3 container.
fn write_epub(doc: &Document, writer: impl Write + Seek) -> Result<(), EpubError> {
    let title = doc.meta.title.clone().unwrap_or_else(|| "Untitled".into());
    let identifier = resolve_identifier(doc);
    let modified_iso = resolve_modified(doc);

    let rendered = content::render_content(doc);
    let content_doc = wrap_content_document(&title, &rendered.body);
    let nav_doc = nav::build_nav_xhtml(&title, &rendered.toc);
    let package = package::build_package_opf(
        &doc.meta,
        &identifier,
        &modified_iso,
        &rendered.images,
        rendered.has_math,
    );

    let mut zip = zip::ZipWriter::new(writer);

    // OCF §4.3: the mimetype entry must be first and stored uncompressed.
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("mimetype", stored)?;
    zip.write_all(container::MIMETYPE.as_bytes())?;

    let deflated =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    write_entry(
        &mut zip,
        container::CONTAINER_PATH,
        container::CONTAINER_XML,
        deflated,
    )?;
    write_entry(&mut zip, container::PACKAGE_PATH, &package, deflated)?;
    write_entry(&mut zip, container::NAV_PATH, &nav_doc, deflated)?;
    write_entry(&mut zip, container::CONTENT_PATH, &content_doc, deflated)?;
    write_entry(
        &mut zip,
        container::STYLE_PATH,
        container::STYLE_CSS,
        deflated,
    )?;

    // Image resources. Already-compressed formats are stored verbatim to avoid
    // wasteful double compression.
    for image in &rendered.images {
        zip.start_file(format!("{}/{}", container::EPUB_DIR, image.href), stored)?;
        zip.write_all(&image.bytes)?;
    }

    zip.finish()?;
    Ok(())
}

fn write_entry<W: Write + Seek>(
    zip: &mut zip::ZipWriter<W>,
    path: &str,
    data: &str,
    options: SimpleFileOptions,
) -> Result<(), EpubError> {
    zip.start_file(path, options)?;
    zip.write_all(data.as_bytes())?;
    Ok(())
}

/// Wraps the body fragment in a complete XHTML5 content document.
fn wrap_content_document(title: &str, body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE html>\n\
         <html xmlns=\"http://www.w3.org/1999/xhtml\" \
         xmlns:epub=\"http://www.idpf.org/2007/ops\" lang=\"en\" xml:lang=\"en\">\n\
         <head>\n  <meta charset=\"utf-8\"/>\n  <title>{title}</title>\n\
         <link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/>\n</head>\n\
         <body>\n{body}</body>\n</html>\n",
        title = xml::escape_text(title),
        body = body,
    )
}

/// Returns the publication identifier: the document's Dublin Core identifier
/// when present, otherwise a synthesised `urn:uuid:` value (EPUB requires a
/// unique identifier).
fn resolve_identifier(doc: &Document) -> String {
    if let Some(id) = doc
        .meta
        .dublin_core
        .identifier
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        return id.to_string();
    }
    format!("urn:uuid:{}", synthesize_uuid(doc))
}

/// Returns the `dcterms:modified` timestamp in `CCYY-MM-DDThh:mm:ssZ` form.
fn resolve_modified(doc: &Document) -> String {
    let dt = doc.meta.modified.unwrap_or_else(chrono::Utc::now);
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Synthesises a version-4-shaped UUID string from the document's identity and
/// the current time. Dependency-free: not cryptographically random, but stable
/// enough to serve as a unique publication identifier.
fn synthesize_uuid(doc: &Document) -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut h1 = DefaultHasher::new();
    doc.meta.title.hash(&mut h1);
    doc.meta.creator.hash(&mut h1);
    nonce.hash(&mut h1);
    let a = h1.finish();

    let mut h2 = DefaultHasher::new();
    a.hash(&mut h2);
    doc.block_count_flat().hash(&mut h2);
    nonce.hash(&mut h2);
    let b = h2.finish();

    // Lay the 128 bits out as 8-4-4-4-12 hex with version (4) and variant (8)
    // nibbles forced into place.
    let time_low = (a >> 32) as u32;
    let time_mid = (a >> 16) as u16;
    let time_hi = 0x4000 | ((a as u16) & 0x0fff);
    let clock = 0x8000 | ((b >> 48) as u16 & 0x3fff);
    let node = b & 0x0000_ffff_ffff_ffff;
    format!("{time_low:08x}-{time_mid:04x}-{time_hi:04x}-{clock:04x}-{node:012x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::content::block::Block;
    use loki_doc_model::content::inline::Inline;
    use std::io::{Cursor, Read};

    fn doc_with_text() -> Document {
        let mut doc = Document::new();
        doc.meta.title = Some("My Book".into());
        doc.meta.creator = Some("Ada".into());
        let sec = doc.first_section_mut().unwrap();
        sec.blocks.clear();
        sec.blocks
            .push(Block::Para(vec![Inline::Str("Hello epub".into())]));
        doc
    }

    fn export_bytes(doc: &Document) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        EpubExport::export(doc, &mut buf, EpubOptions::default()).expect("export");
        buf.into_inner()
    }

    #[test]
    fn produces_valid_zip_with_mimetype_first() {
        let bytes = export_bytes(&doc_with_text());
        // The mimetype must be the first entry and stored uncompressed: the
        // literal media type string appears at byte offset 38 in a stored OCF.
        let archive = zip::ZipArchive::new(Cursor::new(bytes.clone())).expect("zip");
        assert_eq!(archive.file_names().next(), Some("mimetype"));

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        let mut mimetype = String::new();
        archive
            .by_name("mimetype")
            .unwrap()
            .read_to_string(&mut mimetype)
            .unwrap();
        assert_eq!(mimetype, "application/epub+zip");
    }

    #[test]
    fn container_and_package_present() {
        let bytes = export_bytes(&doc_with_text());
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("zip");
        for name in [
            "META-INF/container.xml",
            "EPUB/package.opf",
            "EPUB/nav.xhtml",
            "EPUB/content.xhtml",
        ] {
            assert!(archive.by_name(name).is_ok(), "missing {name}");
        }
        let mut opf = String::new();
        archive
            .by_name("EPUB/package.opf")
            .unwrap()
            .read_to_string(&mut opf)
            .unwrap();
        assert!(opf.contains("<dc:title>My Book</dc:title>"));
        assert!(opf.contains("<dc:creator>Ada</dc:creator>"));
    }

    #[test]
    fn synthesized_uuid_has_version_4_shape() {
        let id = synthesize_uuid(&doc_with_text());
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[2].chars().next(), Some('4'));
    }
}
