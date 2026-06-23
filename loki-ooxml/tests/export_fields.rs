// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Round-trip coverage for body-text dynamic fields: a [`Document`] carrying
//! `Inline::Field` values is exported to DOCX and re-imported, and each
//! field kind must survive (it was previously dropped on export).

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

/// Builds a single-paragraph document whose paragraph contains every field
/// kind we can map, interleaved with plain text.
fn doc_with_fields() -> Document {
    let fields = vec![
        Field::new(FieldKind::PageNumber).with_current_value("3"),
        Field::new(FieldKind::PageCount).with_current_value("9"),
        Field::new(FieldKind::Title),
        Field::new(FieldKind::Author),
        Field::new(FieldKind::Date {
            format: Some("yyyy-MM-dd".to_string()),
        }),
        Field::new(FieldKind::CrossReference {
            target: "_Intro".to_string(),
            format: CrossRefFormat::Page,
        }),
        Field::new(FieldKind::Raw {
            instruction: "MERGEFIELD Name".to_string(),
        }),
    ];

    let mut inlines = vec![Inline::Str("Page ".to_string())];
    for f in fields {
        inlines.push(Inline::Field(f));
        inlines.push(Inline::Space);
    }

    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(inlines)];
    doc
}

/// Collects every `Field` found anywhere in the document body.
fn collect_fields(doc: &Document) -> Vec<Field> {
    fn walk(inlines: &[Inline], out: &mut Vec<Field>) {
        for i in inlines {
            match i {
                Inline::Field(f) => out.push(f.clone()),
                Inline::StyledRun(r) => walk(&r.content, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for section in &doc.sections {
        for block in &section.blocks {
            let inlines = match block {
                Block::Para(i) => i.as_slice(),
                Block::StyledPara(sp) => sp.inlines.as_slice(),
                _ => &[],
            };
            walk(inlines, &mut out);
        }
    }
    out
}

#[test]
fn body_fields_round_trip() {
    let doc = doc_with_fields();

    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut buf, ()).expect("export should succeed");
    let bytes = buf.into_inner();

    let re = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("re-import should succeed");

    let kinds: Vec<FieldKind> = collect_fields(&re.document)
        .into_iter()
        .map(|f| f.kind)
        .collect();

    assert!(
        kinds.contains(&FieldKind::PageNumber),
        "PAGE field must survive export; got {kinds:?}"
    );
    assert!(
        kinds.contains(&FieldKind::PageCount),
        "NUMPAGES must survive"
    );
    assert!(kinds.contains(&FieldKind::Title), "TITLE must survive");
    assert!(kinds.contains(&FieldKind::Author), "AUTHOR must survive");
    assert!(
        kinds.contains(&FieldKind::Date {
            format: Some("yyyy-MM-dd".to_string()),
        }),
        "DATE (with format switch) must survive; got {kinds:?}"
    );
    assert!(
        kinds.contains(&FieldKind::CrossReference {
            target: "_Intro".to_string(),
            format: CrossRefFormat::Page,
        }),
        "PAGEREF cross-reference must survive; got {kinds:?}"
    );
    assert!(
        kinds.iter().any(|k| matches!(
            k,
            FieldKind::Raw { instruction } if instruction.contains("MERGEFIELD")
        )),
        "Raw MERGEFIELD must survive verbatim; got {kinds:?}"
    );
}

#[test]
fn page_number_snapshot_survives() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Field(
        Field::new(FieldKind::PageNumber).with_current_value("42"),
    )])];

    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut buf, ()).expect("export should succeed");

    let re = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import should succeed");

    let fields = collect_fields(&re.document);
    let page = fields
        .iter()
        .find(|f| f.kind == FieldKind::PageNumber)
        .expect("PAGE field must be present");
    assert_eq!(
        page.current_value.as_deref(),
        Some("42"),
        "the cached result snapshot must survive the round-trip"
    );
}
