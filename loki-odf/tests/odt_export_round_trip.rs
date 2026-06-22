// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODT export → import round-trip: a document's styles, page geometry, and
//! metadata must survive being written to an ODT package and read back.

use std::io::Cursor;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::meta::DocumentMeta;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
use loki_odf::odt::export::{OdtExport, OdtExportOptions};
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_primitives::units::Points;

fn para_style(id: &str, name: &str, ch: CharProps, pa: ParaProps) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: Some(name.to_string()),
        parent: (id != "Normal").then(|| StyleId::new("Normal")),
        linked_char_style: None,
        next_style_id: None,
        para_props: pa,
        char_props: ch,
        is_default: id == "Normal",
        is_custom: false,
        extensions: Default::default(),
    }
}

fn sample_doc() -> Document {
    let mut doc = Document::new();

    doc.meta = DocumentMeta {
        title: Some("Round Trip ODT".into()),
        creator: Some("Ada".into()),
        ..Default::default()
    };

    doc.styles.paragraph_styles.insert(
        StyleId::new("Normal"),
        para_style(
            "Normal",
            "Normal",
            CharProps {
                font_name: Some("Times New Roman".into()),
                font_size: Some(Points::new(12.0)),
                ..Default::default()
            },
            ParaProps::default(),
        ),
    );
    doc.styles.paragraph_styles.insert(
        StyleId::new("Quote"),
        para_style(
            "Quote",
            "Quote",
            CharProps {
                italic: Some(true),
                ..Default::default()
            },
            ParaProps {
                indent_start: Some(Points::new(36.0)),
                alignment: Some(ParagraphAlignment::Center),
                ..Default::default()
            },
        ),
    );

    let layout = PageLayout {
        page_size: PageSize::letter(),
        margins: PageMargins {
            top: Points::new(72.0),
            bottom: Points::new(72.0),
            left: Points::new(90.0),
            right: Points::new(72.0),
            ..Default::default()
        },
        ..Default::default()
    };

    let blocks = vec![
        Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("The Title".into())],
        ),
        Block::Para(vec![
            Inline::Str("Plain and ".into()),
            Inline::Strong(vec![Inline::Str("bold".into())]),
            Inline::Str(" text.".into()),
        ]),
        Block::StyledPara(StyledParagraph {
            style_id: Some(StyleId::new("Quote")),
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Str("A quoted line.".into())],
            attr: NodeAttr::default(),
        }),
    ];
    doc.sections = vec![Section::with_layout_and_blocks(layout, blocks)];
    doc
}

fn round_trip(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::<u8>::new());
    OdtExport::export(doc, &mut buf, OdtExportOptions::default()).expect("ODT export");
    OdtImport::import(Cursor::new(buf.into_inner()), OdtImportOptions::default())
        .expect("ODT re-import")
}

#[test]
fn styles_round_trip() {
    let doc = round_trip(&sample_doc());

    let quote = doc
        .styles
        .paragraph_styles
        .get(&StyleId::new("Quote"))
        .expect("Quote style must survive");
    assert_eq!(quote.char_props.italic, Some(true));
    assert_eq!(
        quote.para_props.indent_start.map(|p| p.value().round()),
        Some(36.0)
    );
    assert_eq!(quote.para_props.alignment, Some(ParagraphAlignment::Center));

    let normal = doc
        .styles
        .paragraph_styles
        .get(&StyleId::new("Normal"))
        .expect("Normal style must survive");
    assert_eq!(
        normal.char_props.font_name.as_deref(),
        Some("Times New Roman")
    );
    assert_eq!(
        normal.char_props.font_size.map(|p| p.value().round()),
        Some(12.0)
    );
}

#[test]
fn page_geometry_round_trips() {
    let doc = round_trip(&sample_doc());
    let layout = &doc.sections[0].layout;
    assert_eq!(layout.page_size.width.value().round(), 612.0); // US Letter
    assert_eq!(layout.page_size.height.value().round(), 792.0);
    assert_eq!(layout.margins.left.value().round(), 90.0);
    assert_eq!(layout.margins.top.value().round(), 72.0);
}

#[test]
fn metadata_and_heading_round_trip() {
    let doc = round_trip(&sample_doc());
    assert_eq!(doc.meta.title.as_deref(), Some("Round Trip ODT"));

    let has_heading = doc
        .sections
        .iter()
        .flat_map(|s| &s.blocks)
        .any(|b| matches!(b, Block::Heading(1, _, _)));
    assert!(
        has_heading,
        "the level-1 heading must survive the round-trip"
    );
}
