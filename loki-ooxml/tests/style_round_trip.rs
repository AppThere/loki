// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Style-catalog persistence: a custom paragraph style (and an edited built-in
//! heading) must survive a DOCX export → import round-trip with all of the
//! style-editor-editable fields intact.

use std::io::Cursor;

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{
    LineHeight, ParaProps, ParagraphAlignment, Spacing,
};
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};
use loki_primitives::units::Points;

/// Builds a `Document` carrying a custom paragraph style plus an edited
/// `Heading1`, exports it to DOCX, re-imports, and returns the round-tripped
/// catalog styles.
fn round_trip_styles() -> loki_doc_model::style::catalog::StyleCatalog {
    let mut doc = Document::new();

    // A user-created custom style exercising every editor-editable field.
    let quote = ParagraphStyle {
        id: StyleId::new("MyQuote"),
        display_name: Some("My Quote".to_string()),
        parent: Some(StyleId::new("Normal")),
        linked_char_style: None,
        next_style_id: Some("Normal".to_string()),
        para_props: ParaProps {
            alignment: Some(ParagraphAlignment::Justify),
            indent_start: Some(Points::new(36.0)),
            indent_end: Some(Points::new(24.0)),
            indent_first_line: Some(Points::new(18.0)),
            space_before: Some(Spacing::Exact(Points::new(6.0))),
            space_after: Some(Spacing::Exact(Points::new(12.0))),
            line_height: Some(LineHeight::Multiple(1.5)),
            ..ParaProps::default()
        },
        char_props: CharProps {
            font_name: Some("Arial".to_string()),
            font_size: Some(Points::new(14.0)),
            bold: Some(true),
            ..CharProps::default()
        },
        is_default: false,
        is_custom: true,
        extensions: Default::default(),
    };
    doc.styles
        .paragraph_styles
        .insert(StyleId::new("MyQuote"), quote);

    // An edit to a built-in heading: bump Heading1 to 20pt, centered. This must
    // also persist (export previously hard-coded the headings, dropping edits).
    let heading1 = ParagraphStyle {
        id: StyleId::new("Heading1"),
        display_name: Some("heading 1".to_string()),
        parent: Some(StyleId::new("Normal")),
        linked_char_style: None,
        next_style_id: None,
        para_props: ParaProps {
            alignment: Some(ParagraphAlignment::Center),
            // Model outline level is 1-indexed (1 = Heading 1).
            outline_level: Some(1),
            ..ParaProps::default()
        },
        char_props: CharProps {
            font_size: Some(Points::new(20.0)),
            bold: Some(true),
            ..CharProps::default()
        },
        is_default: false,
        is_custom: false,
        extensions: Default::default(),
    };
    doc.styles
        .paragraph_styles
        .insert(StyleId::new("Heading1"), heading1);

    let mut buf = Cursor::new(Vec::<u8>::new());
    DocxExport::export(&doc, &mut buf, ()).expect("export should succeed");

    let imported = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import should succeed");
    imported.document.styles
}

#[test]
fn custom_paragraph_style_round_trips() {
    let catalog = round_trip_styles();
    let q = catalog
        .paragraph_styles
        .get(&StyleId::new("MyQuote"))
        .expect("custom style MyQuote must survive round-trip");

    assert_eq!(q.display_name.as_deref(), Some("My Quote"));
    assert_eq!(q.parent.as_ref().map(StyleId::as_str), Some("Normal"));
    assert_eq!(q.next_style_id.as_deref(), Some("Normal"));
    assert!(q.is_custom, "custom flag must round-trip");

    let pp = &q.para_props;
    assert_eq!(pp.alignment, Some(ParagraphAlignment::Justify));
    assert_eq!(pp.indent_start, Some(Points::new(36.0)));
    assert_eq!(pp.indent_end, Some(Points::new(24.0)));
    assert_eq!(pp.indent_first_line, Some(Points::new(18.0)));
    assert_eq!(pp.space_before, Some(Spacing::Exact(Points::new(6.0))));
    assert_eq!(pp.space_after, Some(Spacing::Exact(Points::new(12.0))));
    match pp.line_height {
        Some(LineHeight::Multiple(m)) => assert!(
            (m - 1.5).abs() < 0.001,
            "line spacing ratio must round-trip (got {m})"
        ),
        other => panic!("expected Multiple(1.5), got {other:?}"),
    }

    let cp = &q.char_props;
    assert_eq!(cp.font_name.as_deref(), Some("Arial"));
    assert_eq!(cp.font_size, Some(Points::new(14.0)));
    assert_eq!(cp.bold, Some(true));
}

#[test]
fn edited_heading_round_trips() {
    let catalog = round_trip_styles();
    let h1 = catalog
        .paragraph_styles
        .get(&StyleId::new("Heading1"))
        .expect("Heading1 must be present after round-trip");

    // The edited props (20pt, centered) must persist rather than reverting to
    // the hard-coded built-in heading definition.
    assert_eq!(h1.char_props.font_size, Some(Points::new(20.0)));
    assert_eq!(h1.para_props.alignment, Some(ParagraphAlignment::Center));
    assert_eq!(h1.para_props.outline_level, Some(1));
}
