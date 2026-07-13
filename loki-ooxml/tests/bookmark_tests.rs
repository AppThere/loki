// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Regression tests for bookmark ID pairing in DOCX export.
//!
//! ECMA-376 §17.13.6.2 requires that `w:bookmarkStart` and its paired
//! `w:bookmarkEnd` carry the **same** `w:id` attribute value.  A bug in the
//! original exporter incremented the global ID counter independently for each
//! call, producing unmatched pairs that corrupt cross-references in Word.

use std::io::{Cursor, Read};

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{BookmarkKind, Inline};
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::docx::export::DocxExport;
use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

/// Export `doc` to DOCX bytes and return the raw `word/document.xml` content.
fn export_and_get_document_xml(doc: &Document) -> String {
    let mut buf = Cursor::new(Vec::<u8>::new());
    DocxExport::export(doc, &mut buf, ()).expect("export must succeed");
    let bytes = buf.into_inner();

    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("output must be a valid ZIP");
    let mut entry = archive
        .by_name("word/document.xml")
        .expect("word/document.xml must be present");
    let mut content = String::new();
    entry
        .read_to_string(&mut content)
        .expect("document.xml must be valid UTF-8");
    content
}

/// Strip an OOXML namespace prefix (`w:`, `wp:`, etc.) and return the local name.
fn local_name(bytes: &[u8]) -> &[u8] {
    if let Some(pos) = bytes.iter().position(|&b| b == b':') {
        &bytes[pos + 1..]
    } else {
        bytes
    }
}

/// Parse `xml` and collect `(element_local_name, w:id_value)` for every
/// `w:bookmarkStart` and `w:bookmarkEnd` element encountered.
fn collect_bookmark_ids(xml: &str) -> Vec<(String, u32)> {
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut pairs = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let elem_local = local_name(e.local_name().into_inner());
                let is_start = elem_local == b"bookmarkStart";
                let is_end = elem_local == b"bookmarkEnd";
                if is_start || is_end {
                    let elem_name = if is_start {
                        "bookmarkStart"
                    } else {
                        "bookmarkEnd"
                    };
                    for attr in e.attributes().flatten() {
                        if local_name(attr.key.as_ref()) == b"id"
                            && let Ok(val) =
                                attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            && let Ok(id) = val.parse::<u32>()
                        {
                            pairs.push((elem_name.to_string(), id));
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    pairs
}

/// A document with one named bookmark wrapping some text.
///
/// Asserts that the `w:bookmarkStart` and `w:bookmarkEnd` elements in the
/// exported DOCX share the **same** `w:id` attribute value.
#[test]
fn single_bookmark_start_end_ids_match() {
    let mut doc = Document::new();
    let section = doc.sections.first_mut().expect("default section");
    section.blocks.push(Block::Para(vec![
        Inline::Bookmark(BookmarkKind::Start, "section1".into()),
        Inline::Str("Bookmarked text.".into()),
        Inline::Bookmark(BookmarkKind::End, "section1".into()),
    ]));

    let xml = export_and_get_document_xml(&doc);
    let pairs = collect_bookmark_ids(&xml);

    let starts: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "bookmarkStart")
        .map(|(_, id)| *id)
        .collect();
    let ends: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "bookmarkEnd")
        .map(|(_, id)| *id)
        .collect();

    assert_eq!(starts.len(), 1, "expected exactly one bookmarkStart");
    assert_eq!(ends.len(), 1, "expected exactly one bookmarkEnd");
    assert_eq!(
        starts[0], ends[0],
        "bookmarkStart id={} must equal bookmarkEnd id={}",
        starts[0], ends[0]
    );
}

/// Two bookmarks in sequence — each pair must be independently matched.
///
/// Regression guard: the former global counter produced
///   bookmarkStart id=0, bookmarkEnd id=1, bookmarkStart id=2, bookmarkEnd id=3
/// (all unmatched). The correct output is
///   bookmarkStart id=0, bookmarkEnd id=0, bookmarkStart id=1, bookmarkEnd id=1.
#[test]
fn two_bookmarks_ids_independently_matched() {
    let mut doc = Document::new();
    let section = doc.sections.first_mut().expect("default section");
    section.blocks.push(Block::Para(vec![
        Inline::Bookmark(BookmarkKind::Start, "alpha".into()),
        Inline::Str("First.".into()),
        Inline::Bookmark(BookmarkKind::End, "alpha".into()),
        Inline::Space,
        Inline::Bookmark(BookmarkKind::Start, "beta".into()),
        Inline::Str("Second.".into()),
        Inline::Bookmark(BookmarkKind::End, "beta".into()),
    ]));

    let xml = export_and_get_document_xml(&doc);
    let pairs = collect_bookmark_ids(&xml);

    // Collect starts and ends in document order.
    let starts: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "bookmarkStart")
        .map(|(_, id)| *id)
        .collect();
    let ends: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "bookmarkEnd")
        .map(|(_, id)| *id)
        .collect();

    assert_eq!(starts.len(), 2, "expected two bookmarkStart elements");
    assert_eq!(ends.len(), 2, "expected two bookmarkEnd elements");

    // Each start id must appear in ends, and they must be distinct.
    assert!(
        ends.contains(&starts[0]),
        "bookmarkStart id={} has no matching bookmarkEnd",
        starts[0]
    );
    assert!(
        ends.contains(&starts[1]),
        "bookmarkStart id={} has no matching bookmarkEnd",
        starts[1]
    );
    assert_ne!(starts[0], starts[1], "bookmark IDs must be unique");

    // Because both bookmarks are non-nested and sequential, the End IDs
    // must appear in the same order as the Start IDs (LIFO stack pops
    // correctly since names differ).
    assert_eq!(
        starts, ends,
        "start ids {starts:?} must equal end ids {ends:?} in document order"
    );
}

/// A bookmark whose name duplicates another — each pair must still be matched.
///
/// Duplicate bookmark names are valid in OOXML (Word deduplicates by ID).
/// The LIFO stack must handle this correctly.
#[test]
fn duplicate_bookmark_name_ids_independently_matched() {
    let mut doc = Document::new();
    let section = doc.sections.first_mut().expect("default section");

    // Two paragraphs each with the same bookmark name — only legal when they
    // have distinct IDs.
    section.blocks.push(Block::Para(vec![
        Inline::Bookmark(BookmarkKind::Start, "dup".into()),
        Inline::Str("Para one.".into()),
        Inline::Bookmark(BookmarkKind::End, "dup".into()),
    ]));
    section.blocks.push(Block::Para(vec![
        Inline::Bookmark(BookmarkKind::Start, "dup".into()),
        Inline::Str("Para two.".into()),
        Inline::Bookmark(BookmarkKind::End, "dup".into()),
    ]));

    let xml = export_and_get_document_xml(&doc);
    let pairs = collect_bookmark_ids(&xml);

    let starts: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "bookmarkStart")
        .map(|(_, id)| *id)
        .collect();
    let ends: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "bookmarkEnd")
        .map(|(_, id)| *id)
        .collect();

    assert_eq!(starts.len(), 2);
    assert_eq!(ends.len(), 2);

    // Both starts must appear in ends.
    for &sid in &starts {
        assert!(
            ends.contains(&sid),
            "bookmarkStart id={sid} has no matching bookmarkEnd"
        );
    }

    // The two bookmark pairs must have distinct IDs (Word requires uniqueness).
    assert_ne!(
        starts[0], starts[1],
        "duplicate-name bookmark IDs must differ"
    );

    // Verify document-order pairing: first Start → first End, second → second.
    assert_eq!(
        starts, ends,
        "start ids {starts:?} must equal end ids {ends:?} in document order"
    );
}

/// A document with a bookmark spread across a heading and a paragraph,
/// alongside a [`NodeAttr`]-attributed block — exercises the full path
/// through `write_inline` with the collector threading.
#[test]
fn bookmark_in_heading_and_para_ids_match() {
    let mut doc = Document::new();
    let section = doc.sections.first_mut().expect("default section");
    section.blocks.push(Block::Heading(
        1,
        NodeAttr::default(),
        vec![
            Inline::Bookmark(BookmarkKind::Start, "intro".into()),
            Inline::Str("Introduction".into()),
        ],
    ));
    section.blocks.push(Block::Para(vec![
        Inline::Str("Body text.".into()),
        Inline::Bookmark(BookmarkKind::End, "intro".into()),
    ]));

    let xml = export_and_get_document_xml(&doc);
    let pairs = collect_bookmark_ids(&xml);

    let starts: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "bookmarkStart")
        .map(|(_, id)| *id)
        .collect();
    let ends: Vec<u32> = pairs
        .iter()
        .filter(|(k, _)| k == "bookmarkEnd")
        .map(|(_, id)| *id)
        .collect();

    assert_eq!(starts.len(), 1);
    assert_eq!(ends.len(), 1);
    assert_eq!(
        starts[0], ends[0],
        "cross-block bookmark ids must match: start={} end={}",
        starts[0], ends[0]
    );
}
