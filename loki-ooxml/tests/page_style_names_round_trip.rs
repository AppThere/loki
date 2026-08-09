// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! **Page-style names survive a DOCX round trip** via the advisory part
//! (Spec 08 T6.5, decision D-02).
//!
//! OOXML has no named page style: a section's geometry is its `w:sectPr` and
//! nothing else, so before this part a rename made in the style panel was lost
//! the moment the document was saved as `.docx`. These tests go through the real
//! export/import path — a full OPC package written to a buffer and read back —
//! because the whole claim is about what survives the file, not about what two
//! functions agree on in memory.

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::page_style::PageStyle;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn para(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.to_string())])
}

/// Two sections with genuinely different geometry, named `Body` and `Cover`,
/// with a display name on one of them.
fn named_two_section_doc() -> Document {
    let mut doc = Document::new();
    doc.sections = vec![
        Section::with_layout_and_blocks(
            PageLayout {
                page_size: PageSize::a4(),
                orientation: PageOrientation::Portrait,
                ..PageLayout::default()
            },
            vec![para("Section one body")],
        ),
        Section::with_layout_and_blocks(
            PageLayout {
                page_size: PageSize::letter(),
                orientation: PageOrientation::Landscape,
                ..PageLayout::default()
            },
            vec![para("Section two body")],
        ),
    ];
    doc.sections[0].page_style = Some(StyleId::new("Body"));
    doc.sections[1].page_style = Some(StyleId::new("Cover"));
    let mut cover = PageStyle::new(StyleId::new("Cover"), doc.sections[1].layout.clone());
    cover.display_name = Some("Cover Page".to_string());
    doc.styles.page_styles.insert(
        StyleId::new("Body"),
        PageStyle::new(StyleId::new("Body"), doc.sections[0].layout.clone()),
    );
    doc.styles.page_styles.insert(StyleId::new("Cover"), cover);
    doc
}

fn export_bytes(doc: &Document) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("export");
    buf.into_inner()
}

fn import_bytes(bytes: Vec<u8>) -> Document {
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("import")
        .document
}

fn round_trip(doc: Document) -> Document {
    import_bytes(export_bytes(&doc))
}

#[test]
fn page_style_names_survive_a_docx_round_trip() {
    let back = round_trip(named_two_section_doc());

    assert_eq!(back.sections.len(), 2);
    assert_eq!(
        back.sections[0].page_style.as_ref().map(StyleId::as_str),
        Some("Body"),
        "the first section's page-style name was lost"
    );
    assert_eq!(
        back.sections[1].page_style.as_ref().map(StyleId::as_str),
        Some("Cover")
    );
    assert_eq!(
        back.styles
            .page_styles
            .get(&StyleId::new("Cover"))
            .and_then(|p| p.display_name.clone()),
        Some("Cover Page".to_string()),
        "the display name was lost"
    );
}

/// **The advisory part must not move a single point of geometry.** Asserted
/// against the same document exported with the names stripped: whatever the
/// `w:sectPr` round trip does to page size and orientation, it must do
/// identically with and without the part.
#[test]
fn the_advisory_part_changes_no_geometry() {
    let with_names = round_trip(named_two_section_doc());

    let mut plain = named_two_section_doc();
    for s in &mut plain.sections {
        s.page_style = None;
    }
    plain.styles.page_styles.clear();
    let without_names = round_trip(plain);

    assert_eq!(
        with_names.sections.len(),
        without_names.sections.len(),
        "the part changed the section count"
    );
    for (i, (a, b)) in with_names
        .sections
        .iter()
        .zip(without_names.sections.iter())
        .enumerate()
    {
        assert_eq!(
            a.layout, b.layout,
            "section {i}: the advisory part changed the geometry"
        );
    }
    // And the names really were present in one and absent in the other, so the
    // comparison above is not comparing two identical runs.
    assert!(with_names.sections[0].page_style.is_some());
    assert!(
        without_names.sections[0]
            .page_style
            .as_ref()
            .map(StyleId::as_str)
            != Some("Body"),
        "the stripped document came back named anyway"
    );
}

/// A document nobody named exports no part, and imports exactly as it did
/// before T6.5 — the feature costs nothing to documents that do not use it.
#[test]
fn a_document_with_no_names_is_unaffected() {
    let mut doc = named_two_section_doc();
    for s in &mut doc.sections {
        s.page_style = None;
    }
    doc.styles.page_styles.clear();
    let back = round_trip(doc);
    assert_eq!(back.sections.len(), 2);
    assert!(
        back.sections.iter().all(|s| s.page_style.is_none()),
        "names appeared from nowhere"
    );
}

/// **Discarded on mismatch**, end to end: a package whose advisory part
/// describes a different number of sections than the document has must import
/// with no names rather than the wrong ones.
///
/// The part is rewritten inside the real `.docx` to claim three sections, which
/// is what a Word round trip that merged two sections would leave behind.
#[test]
fn a_stale_part_is_discarded_by_the_section_count_check() {
    let buf = export_bytes(&named_two_section_doc());

    // Rewrite the advisory part in place: same package, a count that no longer
    // matches. `sectionCount="2"` → `"3"` is a one-character edit to the ZIP's
    // stored XML, so re-zip through the OPC layer.
    let mut pkg = loki_opc::Package::open(Cursor::new(buf)).expect("open package");
    let name = loki_opc::PartName::new("/word/lokiPageStyles.xml").expect("part name");
    let original = pkg
        .part(&name)
        .expect("the advisory part should exist")
        .bytes
        .clone();
    let text = String::from_utf8(original).expect("utf-8");
    assert!(
        text.contains(r#"sectionCount="2""#),
        "fixture broken — the part does not declare two sections: {text}"
    );
    let stale = text.replace(r#"sectionCount="2""#, r#"sectionCount="3""#);
    let media = pkg.content_type(&name).expect("content type").to_string();
    pkg.set_part(
        name,
        loki_opc::part::PartData::new(stale.into_bytes(), media),
    );
    let mut rezipped = Vec::new();
    pkg.write(&mut Cursor::new(&mut rezipped)).expect("rewrite");

    let back = import_bytes(rezipped);
    assert_eq!(back.sections.len(), 2);
    assert!(
        back.sections
            .iter()
            .all(|s| s.page_style.as_ref().map(StyleId::as_str) != Some("Body")),
        "a stale advisory part was applied anyway"
    );
}
