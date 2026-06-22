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
fn full_character_and_paragraph_props_round_trip() {
    use loki_doc_model::meta::LanguageTag;
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_doc_model::style::props::char_props::{StrikethroughStyle, VerticalAlign};
    use loki_doc_model::style::props::para_props::Spacing;
    use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader, TabStop};
    use loki_primitives::color::DocumentColor;

    let red = DocumentColor::from_hex("#FF0000").unwrap();
    let char_props = CharProps {
        font_name: Some("Arial".into()),
        font_name_complex: Some("Arial Complex".into()),
        font_name_east_asian: Some("MS Mincho".into()),
        font_size: Some(Points::new(14.0)),
        font_size_complex: Some(Points::new(15.0)),
        bold: Some(true),
        italic: Some(true),
        strikethrough: Some(StrikethroughStyle::Single),
        small_caps: Some(true),
        all_caps: Some(true),
        outline: Some(true),
        shadow: Some(true),
        vertical_align: Some(VerticalAlign::Superscript),
        color: Some(red.clone()),
        letter_spacing: Some(Points::new(1.0)),
        word_spacing: Some(Points::new(2.0)),
        kerning: Some(true),
        scale: Some(90.0),
        language: Some(LanguageTag::new("en-GB")),
        language_complex: Some(LanguageTag::new("ar-SA")),
        language_east_asian: Some(LanguageTag::new("ja-JP")),
        ..Default::default()
    };
    let para_props = ParaProps {
        border_top: Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(1.0),
            color: Some(red),
            spacing: None,
        }),
        padding_top: Some(Points::new(6.0)),
        padding_bottom: Some(Points::new(6.0)),
        padding_left: Some(Points::new(6.0)),
        padding_right: Some(Points::new(6.0)),
        tab_stops: Some(vec![TabStop {
            position: Points::new(72.0),
            alignment: TabAlignment::Right,
            leader: TabLeader::Dot,
        }]),
        widow_control: Some(3),
        orphan_control: Some(2),
        bidi: Some(true),
        keep_together: Some(true),
        keep_with_next: Some(true),
        page_break_before: Some(true),
        space_before: Some(Spacing::Exact(Points::new(8.0))),
        ..Default::default()
    };

    let mut doc = sample_doc();
    doc.styles.paragraph_styles.insert(
        StyleId::new("Fancy"),
        para_style("Fancy", "Fancy", char_props, para_props),
    );

    let out = round_trip(&doc);
    let f = out
        .styles
        .paragraph_styles
        .get(&StyleId::new("Fancy"))
        .expect("Fancy style must survive");

    let c = &f.char_props;
    assert_eq!(c.font_name_complex.as_deref(), Some("Arial Complex"));
    assert_eq!(c.font_name_east_asian.as_deref(), Some("MS Mincho"));
    assert_eq!(c.font_size_complex.map(|p| p.value().round()), Some(15.0));
    assert_eq!(c.strikethrough, Some(StrikethroughStyle::Single));
    assert_eq!(c.small_caps, Some(true));
    assert_eq!(c.all_caps, Some(true));
    assert_eq!(c.outline, Some(true));
    assert_eq!(c.shadow, Some(true));
    assert_eq!(c.vertical_align, Some(VerticalAlign::Superscript));
    assert!(c.color.is_some());
    assert_eq!(c.letter_spacing.map(|p| p.value().round()), Some(1.0));
    assert_eq!(c.word_spacing.map(|p| p.value().round()), Some(2.0));
    assert_eq!(c.kerning, Some(true));
    assert_eq!(c.scale, Some(90.0));
    assert_eq!(c.language.as_ref().map(|l| l.as_str()), Some("en-GB"));
    assert_eq!(
        c.language_complex.as_ref().map(|l| l.as_str()),
        Some("ar-SA")
    );
    assert_eq!(
        c.language_east_asian.as_ref().map(|l| l.as_str()),
        Some("ja-JP")
    );

    let p = &f.para_props;
    let border = p.border_top.as_ref().expect("top border survives");
    assert_eq!(border.style, BorderStyle::Solid);
    assert_eq!(border.width.value().round(), 1.0);
    assert_eq!(p.padding_top.map(|v| v.value().round()), Some(6.0));
    let tabs = p.tab_stops.as_ref().expect("tab stops survive");
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].position.value().round(), 72.0);
    assert_eq!(tabs[0].alignment, TabAlignment::Right);
    assert_eq!(tabs[0].leader, TabLeader::Dot);
    assert_eq!(p.widow_control, Some(3));
    assert_eq!(p.orphan_control, Some(2));
    assert_eq!(p.bidi, Some(true));
    assert_eq!(p.keep_together, Some(true));
    assert_eq!(p.keep_with_next, Some(true));
    assert_eq!(p.page_break_before, Some(true));
}

#[test]
fn inline_bookmark_field_and_image_round_trip() {
    use loki_doc_model::content::field::types::{Field, FieldKind};
    use loki_doc_model::content::inline::{BookmarkKind, LinkTarget};

    // A 1x1 transparent PNG, embedded as a data URI (how images live in the model).
    let png_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
    let data_uri = format!("data:image/png;base64,{png_b64}");

    let mut doc = sample_doc();
    doc.sections[0].blocks.push(Block::Para(vec![
        Inline::Bookmark(BookmarkKind::Start, "anchor".into()),
        Inline::Str("see ".into()),
        Inline::Field(Field::new(FieldKind::PageNumber)),
        Inline::Str(" of ".into()),
        Inline::Field(Field::new(FieldKind::PageCount)),
        Inline::Image(
            NodeAttr::default(),
            vec![Inline::Str("a dot".into())],
            LinkTarget::new(data_uri),
        ),
        Inline::Bookmark(BookmarkKind::End, "anchor".into()),
    ]));

    let out = round_trip(&doc);
    let inlines: Vec<&Inline> = out
        .sections
        .iter()
        .flat_map(|s| &s.blocks)
        .flat_map(|b| match b {
            Block::Para(i) | Block::Plain(i) => i.iter().collect(),
            Block::StyledPara(sp) => sp.inlines.iter().collect(),
            _ => Vec::new(),
        })
        .collect();

    assert!(
        inlines
            .iter()
            .any(|i| matches!(i, Inline::Bookmark(BookmarkKind::Start, n) if n == "anchor")),
        "bookmark start must survive"
    );
    assert!(
        inlines
            .iter()
            .any(|i| matches!(i, Inline::Field(f) if matches!(f.kind, FieldKind::PageNumber))),
        "page-number field must survive"
    );
    assert!(
        inlines
            .iter()
            .any(|i| matches!(i, Inline::Field(f) if matches!(f.kind, FieldKind::PageCount))),
        "page-count field must survive"
    );
    let img = inlines
        .iter()
        .find_map(|i| match i {
            Inline::Image(_, _, t) => Some(t),
            _ => None,
        })
        .expect("image must survive the round-trip");
    assert!(
        img.url.starts_with("data:image/png;base64,"),
        "image bytes must round-trip as an embedded data URI, got {}",
        &img.url[..img.url.len().min(32)]
    );
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
