// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for EPUB export (audit T-3): export a document, re-open the
//! OCF ZIP container, and validate its structure — that every XML part is
//! well-formed and that the package's manifest / spine / navigation relationships
//! hold together. The inline unit tests in `lib.rs` cover substring presence;
//! these tests verify the package is actually a coherent, parseable EPUB.

use std::io::{Cursor, Read};

use loki_doc_model::Document;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::io::DocumentExport;
use loki_epub::{EpubExport, EpubOptions};
use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

/// A multi-section document with two headings (so the nav gets a real TOC) and
/// body paragraphs under each.
fn sample_book() -> Document {
    let mut doc = Document::new();
    doc.meta.title = Some("The Loki Compendium".into());
    doc.meta.creator = Some("Ada Lovelace".into());
    let sec = doc.first_section_mut().unwrap();
    sec.blocks.clear();
    sec.blocks.push(Block::Heading(
        1,
        NodeAttr::default(),
        vec![Inline::Str("Chapter One".into())],
    ));
    sec.blocks
        .push(Block::Para(vec![Inline::Str("The opening words.".into())]));
    sec.blocks.push(Block::Heading(
        1,
        NodeAttr::default(),
        vec![Inline::Str("Chapter Two".into())],
    ));
    sec.blocks
        .push(Block::Para(vec![Inline::Str("The closing words.".into())]));
    doc
}

fn export_bytes(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    EpubExport::export(doc, &mut buf, EpubOptions::default()).expect("export");
    buf.into_inner()
}

/// Reads a named entry from the archive into a String.
fn read_entry(archive: &mut ZipArchive<Cursor<Vec<u8>>>, name: &str) -> String {
    let mut s = String::new();
    archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("missing entry {name}"))
        .read_to_string(&mut s)
        .unwrap_or_else(|_| panic!("entry {name} not UTF-8"));
    s
}

/// Drives quick-xml over the whole document, panicking on the first parse error.
/// A clean run to `Eof` proves the part is well-formed XML.
fn assert_well_formed(label: &str, xml: &str) {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("{label} is not well-formed XML: {e}"),
        }
    }
}

#[test]
fn mimetype_is_first_and_stored_uncompressed() {
    let bytes = export_bytes(&sample_book());
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open ocf zip");

    // OCF §4.3: the mimetype entry must be first.
    assert_eq!(archive.file_names().next(), Some("mimetype"));

    let entry = archive.by_name("mimetype").expect("mimetype entry");
    assert_eq!(
        entry.compression(),
        zip::CompressionMethod::Stored,
        "mimetype must be stored uncompressed"
    );
}

#[test]
fn every_xml_part_is_well_formed() {
    let bytes = export_bytes(&sample_book());
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open ocf zip");

    for name in [
        "META-INF/container.xml",
        "EPUB/package.opf",
        "EPUB/nav.xhtml",
        "EPUB/content.xhtml",
    ] {
        let xml = read_entry(&mut archive, name);
        assert_well_formed(name, &xml);
    }
}

#[test]
fn package_manifest_and_spine_are_consistent() {
    let bytes = export_bytes(&sample_book());
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open ocf zip");
    let opf = read_entry(&mut archive, "EPUB/package.opf");

    // Mandatory EPUB 3.3 metadata.
    assert!(opf.contains("<dc:title>The Loki Compendium</dc:title>"));
    assert!(opf.contains("<dc:creator>Ada Lovelace</dc:creator>"));
    assert!(opf.contains("dc:identifier"), "missing dc:identifier");
    assert!(opf.contains("dc:language"), "missing dc:language");
    assert!(opf.contains("dcterms:modified"), "missing dcterms:modified");

    // The nav document must be declared with the `nav` property, and the content
    // document must appear in both the manifest and the spine.
    assert!(
        opf.contains("properties=\"nav\""),
        "nav document not flagged in manifest"
    );
    assert!(
        opf.contains("href=\"content.xhtml\""),
        "content doc missing from manifest"
    );
    assert!(opf.contains("<spine"), "missing spine");
    assert!(
        opf.contains("<itemref"),
        "spine has no itemref to the content document"
    );
}

#[test]
fn nav_lists_both_chapter_headings() {
    let bytes = export_bytes(&sample_book());
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open ocf zip");
    let nav = read_entry(&mut archive, "EPUB/nav.xhtml");

    assert!(
        nav.contains("epub:type=\"toc\""),
        "nav doc missing the toc landmark"
    );
    assert!(nav.contains("Chapter One"), "TOC missing first heading");
    assert!(nav.contains("Chapter Two"), "TOC missing second heading");
}

#[test]
fn content_document_carries_the_body_text() {
    let bytes = export_bytes(&sample_book());
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open ocf zip");
    let content = read_entry(&mut archive, "EPUB/content.xhtml");

    assert!(content.contains("The opening words."));
    assert!(content.contains("The closing words."));
    // Headings should be emitted as XHTML heading elements, not flattened away.
    assert!(content.contains("Chapter One"));
}

#[test]
fn untitled_document_still_produces_a_valid_package() {
    // A blank document (no title/creator) must still yield a spec-valid package
    // with a synthesised identifier and the fallback title.
    let doc = Document::new();
    let bytes = export_bytes(&doc);
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("open ocf zip");

    let opf = read_entry(&mut archive, "EPUB/package.opf");
    assert_well_formed("EPUB/package.opf", &opf);
    assert!(opf.contains("dc:identifier"), "identifier is mandatory");
    assert!(opf.contains("dc:title"), "title is mandatory");
}
