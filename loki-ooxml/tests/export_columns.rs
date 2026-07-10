// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Round-trip coverage for multi-column sections (`w:cols`): a section's
//! column count, gap, and separator flag must survive DOCX export + re-import.

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::layout::page::{PageLayout, SectionColumns};
use loki_doc_model::layout::section::Section;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};
use loki_primitives::units::Points;

fn doc_with_columns(cols: SectionColumns) -> Document {
    let section = Section::with_layout_and_blocks(
        PageLayout {
            columns: Some(cols),
            ..PageLayout::default()
        },
        vec![Block::Para(vec![Inline::Str(
            "Two-column body text".to_string(),
        )])],
    );
    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

fn round_trip(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("export should succeed");
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import should succeed")
        .document
}

#[test]
fn three_columns_with_separator_round_trip() {
    let doc = doc_with_columns(SectionColumns {
        count: 3,
        gap: Points::new(24.0),
        separator: true,
        widths: Vec::new(),
    });

    let cols = round_trip(&doc).sections[0]
        .layout
        .columns
        .clone()
        .expect("columns must survive the round-trip");

    assert_eq!(cols.count, 3, "column count");
    assert_eq!(cols.gap.value().round(), 24.0, "column gap (pt)");
    assert!(cols.separator, "separator line must survive");
}

#[test]
fn two_columns_no_separator_round_trip() {
    let doc = doc_with_columns(SectionColumns::two_column());

    let cols = round_trip(&doc).sections[0]
        .layout
        .columns
        .clone()
        .expect("columns must survive the round-trip");

    assert_eq!(cols.count, 2);
    assert_eq!(cols.gap.value().round(), 18.0);
    assert!(!cols.separator, "no separator was requested");
}

#[test]
fn unequal_column_widths_round_trip() {
    // `w:equalWidth="0"` with an explicit `<w:col w:w=..>` per column: the
    // per-column widths (points) must survive DOCX export + re-import.
    let doc = doc_with_columns(SectionColumns {
        count: 2,
        gap: Points::new(18.0),
        separator: false,
        widths: vec![Points::new(300.0), Points::new(150.0)],
    });

    let cols = round_trip(&doc).sections[0]
        .layout
        .columns
        .clone()
        .expect("columns must survive the round-trip");

    assert_eq!(cols.count, 2, "column count");
    assert_eq!(cols.widths.len(), 2, "both column widths must survive");
    assert_eq!(cols.widths[0].value().round(), 300.0, "first column width");
    assert_eq!(cols.widths[1].value().round(), 150.0, "second column width");
}

#[test]
fn single_column_emits_no_cols() {
    // A layout with no columns must not gain a spurious multi-column layout.
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str("Plain".to_string())])];

    let re = round_trip(&doc);
    assert!(
        re.sections[0].layout.columns.is_none(),
        "single-column document must not produce a w:cols layout"
    );
}
