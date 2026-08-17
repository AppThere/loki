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
fn numeric_font_weight_round_trips_through_odt() {
    // A numeric fo:font-weight (semibold = 600) must survive ODT export. The
    // writer previously emitted fo:font-weight only from `bold`, dropping the
    // numeric weight — import, layout and DOCX export all honour it, so this
    // closes the ODF-export half.
    let mut doc = sample_doc();
    doc.styles.paragraph_styles.insert(
        StyleId::new("Semibold"),
        para_style(
            "Semibold",
            "Semibold",
            CharProps {
                font_weight: Some(600),
                ..Default::default()
            },
            ParaProps::default(),
        ),
    );
    let out = round_trip(&doc);
    let cp = &out
        .styles
        .paragraph_styles
        .get(&StyleId::new("Semibold"))
        .expect("Semibold style must survive")
        .char_props;
    assert_eq!(
        cp.font_weight,
        Some(600),
        "numeric fo:font-weight must survive ODT export"
    );
}

#[test]
fn odf_version_is_preserved_on_export() {
    use loki_doc_model::io::source::DocumentSource;
    // ADR-0002: an ODF 1.1/1.2 file must not be silently upgraded to 1.3 on
    // export. Every ODT part previously hardcoded office:version="1.3", so a
    // 1.1 document came back 1.3 — the round-trip breakage the ADR exists to
    // prevent. (An unknown/non-ODF source still defaults to 1.3.)
    for ver in ["1.1", "1.2"] {
        let mut doc = sample_doc();
        doc.source = Some(DocumentSource::new("odf").with_version(ver));
        let re = round_trip(&doc);
        assert_eq!(
            re.source.and_then(|s| s.version).as_deref(),
            Some(ver),
            "office:version {ver} must survive ODT export"
        );
    }
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
        // scale is a FRACTION (0.9 = 90%), the layout + OOXML `w:w` convention.
        scale: Some(0.9),
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
    assert_eq!(c.scale, Some(0.9));
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

/// Emboss (`style:font-relief="embossed"`) and a character border
/// (`fo:border` + `fo:padding` on `style:text-properties`) must survive an ODT
/// export→re-import. Emboss and imprint share the single `font-relief`
/// attribute, so they are tested in separate styles.
#[test]
fn emboss_and_char_border_round_trip_through_odt() {
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_primitives::color::DocumentColor;

    let embossed = CharProps {
        emboss: Some(true),
        character_border: Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(1.0),
            color: Some(DocumentColor::from_hex("#C00000").unwrap()),
            spacing: Some(Points::new(1.0)),
        }),
        ..Default::default()
    };
    let engraved = CharProps {
        imprint: Some(true),
        ..Default::default()
    };

    let mut doc = sample_doc();
    doc.styles.paragraph_styles.insert(
        StyleId::new("Embossed"),
        para_style("Embossed", "Embossed", embossed, ParaProps::default()),
    );
    doc.styles.paragraph_styles.insert(
        StyleId::new("Engraved"),
        para_style("Engraved", "Engraved", engraved, ParaProps::default()),
    );

    let out = round_trip(&doc);

    let emb = &out
        .styles
        .paragraph_styles
        .get(&StyleId::new("Embossed"))
        .expect("Embossed style survives")
        .char_props;
    assert_eq!(emb.emboss, Some(true), "emboss survives");
    assert_eq!(emb.imprint, None, "emboss is not imprint");
    let border = emb.character_border.as_ref().expect("char border survives");
    assert_eq!(border.style, BorderStyle::Solid);
    assert_eq!(border.width.value().round(), 1.0);
    assert!(border.color.is_some(), "border colour survives");
    assert_eq!(
        border.spacing.map(|p| p.value().round()),
        Some(1.0),
        "border padding survives"
    );

    let eng = &out
        .styles
        .paragraph_styles
        .get(&StyleId::new("Engraved"))
        .expect("Engraved style survives")
        .char_props;
    assert_eq!(eng.imprint, Some(true), "imprint survives");
    assert_eq!(eng.emboss, None, "imprint is not emboss");
}

/// A floating text box (`Inline::TextBox`) must survive an ODT export→re-import:
/// the writer emits a `draw:frame`/`draw:text-box` with a graphic auto-style
/// (wrap + fill + border), and the importer maps that floating frame back to an
/// `Inline::TextBox` — so a text box round-trips DOCX ↔ ODT.
#[test]
fn floating_text_box_round_trips_through_odt() {
    use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};

    let mut attr = NodeAttr::default();
    attr.kv.push(("cx_emu".to_string(), "1828800".to_string())); // 144 pt
    attr.kv.push(("cy_emu".to_string(), "731520".to_string())); // 57.6 pt
    attr.kv
        .push(("textbox-fill".to_string(), "FDF0E6".to_string()));
    attr.kv
        .push(("textbox-line".to_string(), "ED7D31".to_string()));
    FloatWrap {
        wrap: TextWrap::Square,
        side: WrapSide::Right,
        behind_text: false,
    }
    .store(&mut attr);
    let text_box = Inline::TextBox(
        attr,
        vec![Block::Para(vec![Inline::Str("Sidebar body.".to_string())])],
    );

    let mut doc = sample_doc();
    doc.sections[0].blocks.push(Block::Para(vec![
        text_box,
        Inline::Str("Body copy beside the box.".to_string()),
    ]));

    let out = round_trip(&doc);

    let (at, blocks) = out
        .sections
        .iter()
        .flat_map(|s| &s.blocks)
        .flat_map(|b| match b {
            Block::Para(i) | Block::Plain(i) => i.clone(),
            Block::StyledPara(sp) => sp.inlines.clone(),
            _ => vec![],
        })
        .find_map(|i| match i {
            Inline::TextBox(at, blocks) => Some((at, blocks)),
            _ => None,
        })
        .expect("a floating text box must survive as Inline::TextBox");

    assert!(
        at.kv
            .iter()
            .any(|(k, v)| k == "textbox-fill" && v == "FDF0E6"),
        "fill survives: {:?}",
        at.kv
    );
    assert!(
        at.kv
            .iter()
            .any(|(k, v)| k == "textbox-line" && v == "ED7D31"),
        "border survives: {:?}",
        at.kv
    );
    assert!(
        at.kv.iter().any(|(k, v)| k == "cx_emu" && v == "1828800"),
        "geometry survives: {:?}",
        at.kv
    );
    assert!(
        FloatWrap::read_or_class_default(&at).is_some(),
        "wrap survives so the flow engine still floats it: {:?}",
        at.kv
    );
    let inner: String = blocks
        .iter()
        .flat_map(|b| match b {
            Block::Para(i) | Block::Plain(i) => i.clone(),
            Block::StyledPara(sp) => sp.inlines.clone(),
            _ => vec![],
        })
        .filter_map(|i| match i {
            Inline::Str(s) => Some(s),
            _ => None,
        })
        .collect();
    assert!(
        inner.contains("Sidebar body."),
        "interior text survives: {inner:?}"
    );
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
fn headers_and_footers_round_trip() {
    use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};

    let hf = |kind, text: &str| HeaderFooter {
        kind,
        blocks: vec![Block::Para(vec![Inline::Str(text.into())])],
    };

    let mut doc = sample_doc();
    let layout = &mut doc.sections[0].layout;
    layout.header = Some(hf(HeaderFooterKind::Default, "Page header"));
    layout.footer = Some(hf(HeaderFooterKind::Default, "Page footer"));
    layout.header_first = Some(hf(HeaderFooterKind::First, "First-page header"));
    layout.footer_even = Some(hf(HeaderFooterKind::Even, "Even-page footer"));

    let out = round_trip(&doc);
    let layout = &out.sections[0].layout;

    let text_of = |hf: &Option<HeaderFooter>| -> String {
        let Some(h) = hf else {
            return String::new();
        };
        let Some(block) = h.blocks.first() else {
            return String::new();
        };
        let inl: &[Inline] = match block {
            Block::Para(i) | Block::Plain(i) => i,
            Block::StyledPara(sp) => &sp.inlines,
            _ => return String::new(),
        };
        inl.iter()
            .filter_map(|i| match i {
                Inline::Str(s) => Some(s.as_str()),
                _ => None,
            })
            .collect()
    };

    assert_eq!(text_of(&layout.header), "Page header");
    assert_eq!(text_of(&layout.footer), "Page footer");
    assert_eq!(text_of(&layout.header_first), "First-page header");
    assert_eq!(text_of(&layout.footer_even), "Even-page footer");
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

#[test]
fn multi_section_page_geometry_round_trips() {
    use loki_doc_model::layout::page::PageOrientation;

    let body = |t: &str| vec![Block::Para(vec![Inline::Str(t.to_string())])];

    let portrait = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::a4(),
            orientation: PageOrientation::Portrait,
            ..PageLayout::default()
        },
        body("First section, portrait A4."),
    );
    let landscape = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::letter(),
            orientation: PageOrientation::Landscape,
            ..PageLayout::default()
        },
        body("Second section, landscape Letter."),
    );

    let mut doc = Document::new();
    doc.sections = vec![portrait, landscape];

    let re = round_trip(&doc);

    assert_eq!(
        re.sections.len(),
        2,
        "both sections must survive the round-trip"
    );

    // Each section keeps its own page-layout. ODF records orientation as a
    // flag without swapping the stored width/height, so dimensions round-trip
    // verbatim alongside the orientation.
    let s0 = &re.sections[0].layout;
    assert_eq!(s0.orientation, PageOrientation::Portrait);
    assert_eq!(s0.page_size.width.value().round(), 595.0); // A4
    assert_eq!(s0.page_size.height.value().round(), 842.0);

    let s1 = &re.sections[1].layout;
    assert_eq!(s1.orientation, PageOrientation::Landscape);
    assert_eq!(s1.page_size.width.value().round(), 612.0); // US Letter
    assert_eq!(s1.page_size.height.value().round(), 792.0);

    // The body text of both sections must be preserved.
    let all_text: String = re
        .sections
        .iter()
        .flat_map(|s| &s.blocks)
        .map(text_of_block)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(all_text.contains("First section"), "got: {all_text}");
    assert!(all_text.contains("Second section"), "got: {all_text}");
}

#[test]
fn named_page_styles_round_trip_as_master_pages() {
    use loki_doc_model::layout::page::PageOrientation;
    use loki_doc_model::style::PageStyle;

    let body = |t: &str| vec![Block::Para(vec![Inline::Str(t.to_string())])];

    let mut cover = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::a4(),
            orientation: PageOrientation::Portrait,
            ..PageLayout::default()
        },
        body("Cover section."),
    );
    cover.page_style = Some(StyleId::new("Cover"));
    let mut landscape = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::letter(),
            orientation: PageOrientation::Landscape,
            ..PageLayout::default()
        },
        body("Wide section."),
    );
    landscape.page_style = Some(StyleId::new("WideBody"));

    let mut doc = Document::new();
    doc.sections = vec![cover, landscape];
    // The catalog carries the named page styles (as the app populates them).
    doc.styles.page_styles.insert(
        StyleId::new("Cover"),
        PageStyle::new(StyleId::new("Cover"), doc.sections[0].layout.clone()),
    );
    // "WideBody" carries a distinct human display name (with a space, so the id
    // is the NCName and the display name is separate).
    let mut wide = PageStyle::new(StyleId::new("WideBody"), doc.sections[1].layout.clone());
    wide.display_name = Some("Wide Body".to_string());
    doc.styles
        .page_styles
        .insert(StyleId::new("WideBody"), wide);

    let re = round_trip(&doc);

    // The stored per-section page-style names survive as ODT master pages.
    assert_eq!(re.sections.len(), 2, "both sections must survive");
    assert_eq!(
        re.sections[0].page_style,
        Some(StyleId::new("Cover")),
        "section 0 must keep its page-style name"
    );
    assert_eq!(
        re.sections[1].page_style,
        Some(StyleId::new("WideBody")),
        "section 1 must keep its page-style name"
    );
    // Import registers them as first-class page styles in the catalog.
    assert!(re.styles.page_styles.contains_key(&StyleId::new("Cover")));
    assert!(
        re.styles
            .page_styles
            .contains_key(&StyleId::new("WideBody"))
    );
    // The distinct display name survives via `style:display-name`; the style
    // with no distinct display name leaves it unset (the id is the label).
    assert_eq!(
        re.styles
            .page_styles
            .get(&StyleId::new("WideBody"))
            .and_then(|ps| ps.display_name.as_deref()),
        Some("Wide Body"),
    );
    assert_eq!(
        re.styles
            .page_styles
            .get(&StyleId::new("Cover"))
            .and_then(|ps| ps.display_name.as_deref()),
        None,
    );
    // Geometry still round-trips under the named styles.
    assert_eq!(
        re.sections[1].layout.orientation,
        PageOrientation::Landscape
    );
}

#[test]
fn extended_dublin_core_round_trips() {
    use loki_doc_model::meta::dublin_core::DublinCoreMeta;

    let dc = DublinCoreMeta {
        contributors: vec!["Editor One".into(), "Translator Two".into()],
        publisher: Some("AppThere Press".into()),
        rights: Some("© 2026 AppThere".into()),
        license: Some("https://creativecommons.org/licenses/by/4.0/".into()),
        identifier: Some("urn:uuid:1234".into()),
        identifier_scheme: Some("UUID".into()),
        dc_type: Some("Text".into()),
        format: Some("application/vnd.oasis.opendocument.text".into()),
        source: Some("Original".into()),
        relation: Some("Companion".into()),
        coverage: Some("2026".into()),
        issued: Some("2026-06-22".into()),
        bibliographic_citation: Some("AppThere (2026)".into()),
    };
    let mut doc = Document::new();
    doc.meta.title = Some("DC Doc".into());
    doc.meta.dublin_core = dc.clone();

    let re = round_trip(&doc);
    assert_eq!(
        re.meta.dublin_core, dc,
        "all extended Dublin Core fields must survive the ODT round-trip"
    );
}

#[test]
fn comments_round_trip() {
    use loki_doc_model::content::annotation::{Comment, CommentRef, CommentRefKind};

    let para = Block::Para(vec![
        Inline::Str("Hello ".into()),
        Inline::Comment(CommentRef::new("c1", CommentRefKind::Start)),
        Inline::Str("world".into()),
        Inline::Comment(CommentRef::new("c1", CommentRefKind::End)),
    ]);
    let mut comment = Comment::new("c1").with_plain_body("Please rephrase.\nAnd shorten it.");
    comment.author = Some("Reviewer".into());

    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para];
    doc.comments = vec![comment];

    let re = round_trip(&doc);

    // Anchors survive in the content flow.
    let kinds: Vec<CommentRefKind> = re
        .sections
        .iter()
        .flat_map(|s| &s.blocks)
        .flat_map(|b| match b {
            Block::Para(i) | Block::Plain(i) => i.clone(),
            Block::StyledPara(sp) => sp.inlines.clone(),
            _ => Vec::new(),
        })
        .filter_map(|i| {
            if let Inline::Comment(c) = i {
                Some(c.kind)
            } else {
                None
            }
        })
        .collect();
    assert!(
        kinds.contains(&CommentRefKind::Start),
        "start anchor: {kinds:?}"
    );
    assert!(
        kinds.contains(&CommentRefKind::End),
        "end anchor: {kinds:?}"
    );

    // The comment body + author survive.
    assert_eq!(re.comments.len(), 1, "one comment expected");
    let c = &re.comments[0];
    assert_eq!(c.id, "c1");
    assert_eq!(c.author.as_deref(), Some("Reviewer"));
    let texts: Vec<String> = c
        .body
        .iter()
        .map(|b| match b {
            Block::Para(i) | Block::Plain(i) => i
                .iter()
                .filter_map(|x| {
                    if let Inline::Str(s) = x {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .collect(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(texts, vec!["Please rephrase.", "And shorten it."]);
}

#[test]
fn multi_column_section_round_trips() {
    use loki_doc_model::layout::page::SectionColumns;

    let section = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::a4(),
            columns: Some(SectionColumns {
                count: 3,
                gap: Points::new(18.0),
                separator: true,
                widths: Vec::new(),
            }),
            ..PageLayout::default()
        },
        vec![Block::Para(vec![Inline::Str("Three-column body.".into())])],
    );
    let mut doc = Document::new();
    doc.sections = vec![section];

    let re = round_trip(&doc);
    let cols = re.sections[0]
        .layout
        .columns
        .clone()
        .expect("style:columns must survive the round-trip");

    assert_eq!(cols.count, 3, "column count");
    assert_eq!(cols.gap.value().round(), 18.0, "column gap (pt)");
    assert!(cols.separator, "the column separator must survive");
}

#[test]
fn unequal_column_widths_round_trip() {
    use loki_doc_model::layout::page::SectionColumns;

    // A4 (595.28pt) with 72pt L/R margins → content width 451.28. Two columns
    // in a 2:1 ratio. ODF stores relative `style:column @style:rel-width`, so the
    // absolute widths are re-derived from the page geometry on import — the
    // *ratio* is what round-trips, not the exact points.
    let section = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::a4(),
            columns: Some(SectionColumns {
                count: 2,
                gap: Points::new(18.0),
                separator: false,
                widths: vec![Points::new(280.0), Points::new(140.0)],
            }),
            ..PageLayout::default()
        },
        vec![Block::Para(vec![Inline::Str("Two-column body.".into())])],
    );
    let mut doc = Document::new();
    doc.sections = vec![section];

    let re = round_trip(&doc);
    let cols = re.sections[0]
        .layout
        .columns
        .clone()
        .expect("style:columns must survive the round-trip");

    assert_eq!(cols.count, 2, "column count");
    assert_eq!(cols.widths.len(), 2, "per-column widths survive");
    let ratio = cols.widths[0].value() / cols.widths[1].value();
    assert!(
        (ratio - 2.0).abs() < 0.05,
        "the 2:1 width ratio must survive (got {ratio})"
    );
}

#[test]
fn no_columns_means_no_column_layout() {
    // The default sample document is single-column; it must not gain a
    // spurious multi-column layout on round-trip.
    let doc = round_trip(&sample_doc());
    assert!(
        doc.sections[0].layout.columns.is_none(),
        "single-column document must not produce a style:columns layout"
    );
}

/// Extracts the concatenated plain text of a paragraph-like block.
fn text_of_block(block: &Block) -> String {
    let inlines = match block {
        Block::Para(i) | Block::Plain(i) => i.as_slice(),
        Block::StyledPara(sp) => sp.inlines.as_slice(),
        _ => &[],
    };
    inlines
        .iter()
        .filter_map(|i| {
            if let Inline::Str(s) = i {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

/// A cell with a background colour must survive ODT export → import: the
/// writer emits an automatic `table-cell` style carrying `fo:background-color`,
/// referenced by the cell's `table:style-name`. This is ODF's per-cell
/// representation of table shading / banding.
#[test]
fn cell_background_round_trips_via_table_cell_style() {
    use loki_doc_model::content::table::core::{Table, TableBody, TableFoot, TableHead};
    use loki_doc_model::content::table::row::{Cell, CellProps, Row};
    use loki_primitives::color::{DocumentColor, RgbColor};

    let blue = DocumentColor::Rgb(RgbColor::new(
        0x44 as f32 / 255.0,
        0x72 as f32 / 255.0,
        0xC4 as f32 / 255.0,
    ));

    // Row 1: shaded cell + plain cell. Row 2: both plain.
    let shaded = Cell {
        props: CellProps {
            background_color: Some(blue),
            ..CellProps::default()
        },
        ..Cell::simple(vec![Block::Para(vec![Inline::Str("hdr".into())])])
    };
    let plain = |t: &str| Cell::simple(vec![Block::Para(vec![Inline::Str(t.into())])]);
    let table = Table {
        attr: NodeAttr::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![
            Row::new(vec![shaded, plain("b")]),
            Row::new(vec![plain("c"), plain("d")]),
        ])],
        foot: TableFoot::empty(),
    };

    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = round_trip(&doc);

    let t = back.sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .expect("table survives");
    let first_cell = &t.bodies[0].body_rows[0].cells[0];
    assert_eq!(
        first_cell
            .props
            .background_color
            .as_ref()
            .and_then(DocumentColor::to_hex)
            .as_deref(),
        Some("#4472C4"),
        "shaded cell keeps its background"
    );
    // The plain neighbour stays unshaded.
    assert!(
        t.bodies[0].body_rows[0].cells[1]
            .props
            .background_color
            .is_none()
    );
}

/// A table referencing a banded style (with no direct cell shading) exports to
/// ODT with the banding **resolved into per-cell backgrounds** (ODF's model),
/// so the header row comes back shaded while the body row does not.
#[test]
fn table_style_banding_resolves_into_per_cell_shading_on_odt_export() {
    use loki_doc_model::content::table::core::{Table, TableBody, TableFoot, TableHead};
    use loki_doc_model::content::table::row::{Cell, Row};
    use loki_doc_model::style::table_style::{
        TableConditionalFormat, TableProps, TableRegion, TableStyle,
    };
    use loki_primitives::color::{DocumentColor, RgbColor};

    let blue = DocumentColor::Rgb(RgbColor::new(
        0x44 as f32 / 255.0,
        0x72 as f32 / 255.0,
        0xC4 as f32 / 255.0,
    ));

    // Style with header-row shading only.
    let mut style = TableStyle {
        id: StyleId::new("Banded"),
        display_name: Some("Banded".into()),
        parent: None,
        table_props: TableProps::default(),
        conditional: Default::default(),
        extensions: Default::default(),
    };
    style.conditional.insert(
        TableRegion::FirstRow,
        TableConditionalFormat {
            background_color: Some(blue.clone()),
            char_props: loki_doc_model::style::props::char_props::CharProps::default(),
        },
    );

    // A 2×2 table with no direct cell shading, referencing "Banded".
    let cell = |t: &str| Cell::simple(vec![Block::Para(vec![Inline::Str(t.into())])]);
    let mut table = Table {
        attr: NodeAttr::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![
            Row::new(vec![cell("a"), cell("b")]),
            Row::new(vec![cell("c"), cell("d")]),
        ])],
        foot: TableFoot::empty(),
    };
    table.set_style_name(Some("Banded".into()));

    let mut doc = Document::new();
    doc.styles
        .table_styles
        .insert(StyleId::new("Banded"), style);
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = round_trip(&doc);

    let t = back.sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .expect("table survives");
    let hex = |c: &Cell| {
        c.props
            .background_color
            .as_ref()
            .and_then(DocumentColor::to_hex)
    };
    // Header row (row 0): both cells shaded blue by the resolved firstRow band.
    assert_eq!(
        hex(&t.bodies[0].body_rows[0].cells[0]).as_deref(),
        Some("#4472C4")
    );
    assert_eq!(
        hex(&t.bodies[0].body_rows[0].cells[1]).as_deref(),
        Some("#4472C4")
    );
    // Body row (row 1): no matching region → no shading.
    assert!(
        t.bodies[0].body_rows[1].cells[0]
            .props
            .background_color
            .is_none()
    );
}

/// A table's named-style reference (`table:style-name`) **and its
/// definition** survive ODT export → import: the writer emits the
/// `<style:style style:family="table">` with its `style:table-properties`
/// (width / alignment / background) into styles.xml plus the reference on the
/// table; import restores `style_name` and re-reads the definition into the
/// catalog's `table_styles` entry (deferred-features 4a.3 tail).
#[test]
fn table_style_name_reference_round_trips() {
    use loki_doc_model::content::table::core::Table;
    use loki_doc_model::style::table_style::{TableAlignment, TableProps, TableStyle, TableWidth};
    use loki_primitives::color::DocumentColor;
    use loki_primitives::units::Points;

    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("Banded".into()));

    let mut doc = Document::new();
    doc.styles.table_styles.insert(
        StyleId::new("Banded"),
        TableStyle {
            id: StyleId::new("Banded"),
            display_name: Some("Banded".into()),
            parent: None,
            table_props: TableProps {
                width: Some(TableWidth::Absolute(Points::new(340.0))),
                alignment: Some(TableAlignment::Center),
                background_color: Some(DocumentColor::from_hex("#CADCFC").unwrap()),
                ..TableProps::default()
            },
            conditional: Default::default(),
            extensions: Default::default(),
        },
    );
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = round_trip(&doc);

    let t = back.sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .expect("table survives");
    assert_eq!(t.style_name(), Some("Banded"));

    // The definition round-trips into the catalog, not just the reference.
    let style = back
        .styles
        .table_styles
        .get(&StyleId::new("Banded"))
        .expect("table style definition re-imported");
    match style.table_props.width {
        Some(TableWidth::Absolute(w)) => assert!((w.value() - 340.0).abs() < 0.01),
        ref other => panic!("expected absolute width, got {other:?}"),
    }
    assert_eq!(style.table_props.alignment, Some(TableAlignment::Center));
    assert_eq!(
        style
            .table_props
            .background_color
            .as_ref()
            .and_then(DocumentColor::to_hex)
            .as_deref(),
        Some("#CADCFC")
    );
}

/// A percent-width table style survives via `style:rel-width`.
#[test]
fn table_style_percent_width_round_trips() {
    use loki_doc_model::content::table::core::Table;
    use loki_doc_model::style::table_style::{TableProps, TableStyle, TableWidth};

    let mut table = Table::grid(1, 1);
    table.set_style_name(Some("Half".into()));
    let mut doc = Document::new();
    doc.styles.table_styles.insert(
        StyleId::new("Half"),
        TableStyle {
            id: StyleId::new("Half"),
            display_name: None,
            parent: None,
            table_props: TableProps {
                width: Some(TableWidth::Percent(50.0)),
                ..TableProps::default()
            },
            conditional: Default::default(),
            extensions: Default::default(),
        },
    );
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = round_trip(&doc);
    let style = back
        .styles
        .table_styles
        .get(&StyleId::new("Half"))
        .expect("table style re-imported");
    match style.table_props.width {
        Some(TableWidth::Percent(p)) => assert!((p - 50.0).abs() < 0.01),
        ref other => panic!("expected percent width, got {other:?}"),
    }
}

/// 5.10: the document-wide defaults survive export as `<style:default-style>`
/// elements — the paragraph family from the catalog's `is_default` style and
/// the text family from `default_character_style` — and read back as the
/// synthetic `__Default` / `__DefaultChar` entries. Before this, the defaults
/// were silently dropped (the named-style writer skips synthetic styles), so a
/// re-opened document lost its base font and size.
#[test]
fn default_styles_round_trip() {
    use loki_doc_model::style::char_style::CharacterStyle;

    let mut doc = sample_doc();
    // A text-family default alongside sample_doc's default "Normal" (Times 12).
    let ch_id = StyleId::new("__DefaultChar");
    doc.styles.character_styles.insert(
        ch_id.clone(),
        CharacterStyle {
            id: ch_id.clone(),
            display_name: None,
            parent: None,
            char_props: CharProps {
                italic: Some(true),
                ..Default::default()
            },
            extensions: Default::default(),
        },
    );
    doc.styles.default_character_style = Some(ch_id);

    let mut buf = Cursor::new(Vec::new());
    OdtExport::export(&doc, &mut buf, OdtExportOptions::default()).expect("export");
    let back = OdtImport::import(Cursor::new(buf.into_inner()), OdtImportOptions::default())
        .expect("import");

    let default = back
        .styles
        .paragraph_styles
        .values()
        .find(|s| s.is_default)
        .expect("a default paragraph style must survive the round trip");
    assert_eq!(
        default.char_props.font_name.as_deref(),
        Some("Times New Roman"),
        "the default base font must round-trip"
    );
    assert_eq!(
        default.char_props.font_size.map(|p| p.value()),
        Some(12.0),
        "the default base size must round-trip"
    );
    assert_eq!(
        back.styles.default_paragraph_style.as_ref(),
        Some(&default.id),
        "the catalog default wiring must point at the imported default style"
    );

    let ch = back
        .styles
        .default_character_style
        .as_ref()
        .and_then(|id| back.styles.character_styles.get(id))
        .expect("the text-family default must survive the round trip");
    assert_eq!(
        ch.char_props.italic,
        Some(true),
        "the text-family default's properties must round-trip"
    );
}

/// 4a.3: direct cell borders and padding round-trip through the per-cell
/// automatic `table-cell` style — export previously baked only the
/// background, dropping borders and padding a DOCX-imported (or edited)
/// table carried.
#[test]
fn cell_borders_and_padding_round_trip_via_cell_style() {
    use loki_doc_model::content::table::core::{Table, TableBody, TableFoot, TableHead};
    use loki_doc_model::content::table::row::{Cell, CellProps, Row};
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_primitives::color::DocumentColor;

    let bordered = Cell {
        props: CellProps {
            border_top: Some(Border::solid(
                Points::new(1.0),
                DocumentColor::from_hex("#FF0000").unwrap(),
            )),
            border_left: Some(Border {
                style: BorderStyle::Dashed,
                width: Points::new(0.5),
                color: None,
                spacing: None,
            }),
            padding_top: Some(Points::new(4.0)),
            padding_right: Some(Points::new(6.0)),
            ..CellProps::default()
        },
        ..Cell::simple(vec![Block::Para(vec![Inline::Str("x".into())])])
    };
    let plain = Cell::simple(vec![Block::Para(vec![Inline::Str("y".into())])]);
    let table = Table {
        attr: NodeAttr::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![bordered, plain])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = round_trip(&doc);
    let t = back.sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Table(t) => Some(t.as_ref()),
            _ => None,
        })
        .expect("table survives");
    let cell = &t.bodies[0].body_rows[0].cells[0];

    let top = cell.props.border_top.as_ref().expect("top border survives");
    assert_eq!(top.style, BorderStyle::Solid);
    assert!((top.width.value() - 1.0).abs() < 0.05);
    assert_eq!(
        top.color
            .as_ref()
            .and_then(DocumentColor::to_hex)
            .as_deref(),
        Some("#FF0000")
    );
    assert_eq!(
        cell.props.border_left.as_ref().map(|b| b.style),
        Some(BorderStyle::Dashed)
    );
    assert!((cell.props.padding_top.expect("padding-top").value() - 4.0).abs() < 0.05);
    assert!((cell.props.padding_right.expect("padding-right").value() - 6.0).abs() < 0.05);
    let plain_back = &t.bodies[0].body_rows[0].cells[1];
    assert!(
        plain_back.props.border_top.is_none(),
        "plain cell stays clean"
    );
}

// ── Page usage (Spec 08 T6.1) ────────────────────────────────────────────────

/// Exports `doc` and returns the raw `styles.xml`, so a test can assert what a
/// *conforming third-party reader* would see rather than only what ours does.
fn exported_styles_xml(doc: &Document) -> String {
    let mut buf = Cursor::new(Vec::<u8>::new());
    OdtExport::export(doc, &mut buf, OdtExportOptions::default()).expect("ODT export");
    let bytes = buf.into_inner();
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("odt is a zip");
    let mut f = zip.by_name("styles.xml").expect("styles.xml");
    let mut out = String::new();
    std::io::Read::read_to_string(&mut f, &mut out).expect("read styles.xml");
    out
}

fn one_section(usage: loki_doc_model::layout::page::PageUsage) -> Document {
    let mut doc = Document::new();
    doc.sections = vec![Section::with_layout_and_blocks(
        PageLayout {
            page_usage: usage,
            ..PageLayout::default()
        },
        vec![Block::Para(vec![Inline::Str("x".into())])],
    )];
    doc
}

/// **Mirrored margins survive the ODT round-trip**, which they could not before:
/// the writer emitted no `style:page-usage` and the reader looked for none, so a
/// mirrored document exported as single-sided and re-imported that way. Nothing
/// failed — the geometry was all still there, only the property that alternates
/// it was gone.
///
/// Asserted on the **bytes** as well as on the re-import. A reader and writer
/// that agreed on some other spelling would round-trip perfectly through each
/// other and be wrong for every other application, which is the failure a
/// round-trip test alone cannot see.
#[test]
fn mirrored_page_usage_survives_an_odt_round_trip() {
    use loki_doc_model::layout::page::PageUsage;

    let doc = one_section(PageUsage::Mirrored);
    let xml = exported_styles_xml(&doc);
    assert!(
        xml.contains(r#"style:page-usage="mirrored""#),
        "the ODF attribute is missing from styles.xml:\n{xml}",
    );

    let back = round_trip(&doc);
    assert_eq!(
        back.sections[0].layout.page_usage,
        PageUsage::Mirrored,
        "style:page-usage was dropped between the writer and the reader",
    );
    assert!(
        back.mirrors_margins(),
        "the document-level question must answer yes from the layouts alone — \
         an ODT carries no `settings.mirror_margins`",
    );
}

/// **The polarity, and it is the half that catches a writer emitting the
/// attribute unconditionally.** An ordinary document must not carry it at all —
/// `all` is ODF's default, and writing it would change the bytes of every
/// document this suite has ever produced.
#[test]
fn an_ordinary_document_carries_no_page_usage_at_all() {
    use loki_doc_model::layout::page::PageUsage;

    let doc = one_section(PageUsage::All);
    let xml = exported_styles_xml(&doc);
    assert!(
        !xml.contains("style:page-usage"),
        "the default must not be written:\n{xml}",
    );

    let back = round_trip(&doc);
    assert_eq!(back.sections[0].layout.page_usage, PageUsage::All);
    assert!(!back.mirrors_margins());
}

/// **`left` and `right` are not `mirrored`**, carried through the real writer
/// and reader rather than only through the codec's unit test. A reader that
/// mapped every non-`all` value to mirrored would pass both tests above.
#[test]
fn the_selecting_usages_round_trip_without_becoming_mirrored() {
    use loki_doc_model::layout::page::PageUsage;

    for usage in [PageUsage::Left, PageUsage::Right] {
        let back = round_trip(&one_section(usage));
        assert_eq!(back.sections[0].layout.page_usage, usage, "{usage:?}");
        assert!(
            !back.mirrors_margins(),
            "{usage:?} selects which pages a layout is used for; it does not \
             alternate margins",
        );
    }
}

/// Gap #1 — headings: a heading's own style name must survive ODT export.
///
/// `Block::Heading` names its style in `NodeAttr` under `"style"` (the ODF
/// mapper puts `text:style-name` there), not in a typed field. The ODT writer
/// ignored it and synthesised `Heading{level}` from the level instead, so an
/// imported `Heading_20_1` came back out as `Heading1` — a name the written
/// `styles.xml` does not declare. The reference dangled and the heading lost
/// its formatting (18 pt bold → nothing) on every save.
///
/// The discriminating assertion is the last one: the style the body references
/// must actually be present in the catalog that ships with it.
#[test]
fn gap1_heading_style_survives_odt_export() {
    let mut doc = sample_doc();
    doc.styles.paragraph_styles.insert(
        StyleId::new("Heading_20_1"),
        para_style(
            "Heading_20_1",
            "Heading 1",
            CharProps {
                font_size: Some(Points::new(18.0)),
                bold: Some(true),
                ..Default::default()
            },
            ParaProps::default(),
        ),
    );
    let mut attr = NodeAttr::default();
    attr.kv
        .push(("style".to_string(), "Heading_20_1".to_string()));
    doc.sections[0].blocks.insert(
        0,
        Block::Heading(1, attr, vec![Inline::Str("Chapter One".into())]),
    );

    let back = round_trip(&doc);

    let (level, attr) = back.sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Heading(l, a, _) => Some((*l, a.clone())),
            _ => None,
        })
        .expect("heading must survive export");
    assert_eq!(level, 1);

    let referenced = attr
        .kv
        .iter()
        .find(|(k, _)| k == "style")
        .map(|(_, v)| v.clone())
        .expect("heading must carry a style name");
    assert_eq!(
        referenced, "Heading_20_1",
        "the heading's own style name must be written, not one synthesised \
         from the outline level"
    );

    // The property that actually broke: the reference must resolve.
    let style = back
        .styles
        .paragraph_styles
        .get(&StyleId::new(&referenced))
        .unwrap_or_else(|| {
            panic!("body references '{referenced}', which styles.xml does not declare")
        });
    assert_eq!(style.char_props.bold, Some(true), "heading must stay bold");
    let pt = style.char_props.font_size.map(|p| p.value() as f32);
    assert!(
        pt.is_some_and(|v| (v - 18.0).abs() < 0.5),
        "heading must stay 18 pt, got {pt:?}"
    );
}

/// Gap #1, fallback half: a heading that carries no style name still gets the
/// canonical `Heading{level}` — the in-app-authored case, whose output must
/// not change.
#[test]
fn gap1_heading_without_a_carried_style_falls_back_to_the_canonical_name() {
    let mut doc = sample_doc();
    doc.sections[0].blocks.insert(
        0,
        Block::Heading(
            2,
            NodeAttr::default(),
            vec![Inline::Str("Plain Heading".into())],
        ),
    );

    let back = round_trip(&doc);
    let attr = back.sections[0]
        .blocks
        .iter()
        .find_map(|b| match b {
            Block::Heading(2, a, _) => Some(a.clone()),
            _ => None,
        })
        .expect("heading must survive export");
    let referenced = attr
        .kv
        .iter()
        .find(|(k, _)| k == "style")
        .map(|(_, v)| v.as_str().to_string());
    assert_eq!(
        referenced.as_deref(),
        Some("Heading2"),
        "an unstyled heading keeps the canonical fallback name"
    );
}

/// Gap #1, second writer path: a heading that opens a non-first section is
/// written by `write_block_with_master` (it carries `style:master-page-name`),
/// which had the same synthesised-name bug and its own code path.
#[test]
fn gap1_heading_style_survives_when_it_opens_a_later_section() {
    let mut doc = sample_doc();
    doc.styles.paragraph_styles.insert(
        StyleId::new("SceneHeading"),
        para_style(
            "SceneHeading",
            "Scene Heading",
            CharProps {
                bold: Some(true),
                ..Default::default()
            },
            ParaProps::default(),
        ),
    );
    let mut attr = NodeAttr::default();
    attr.kv
        .push(("style".to_string(), "SceneHeading".to_string()));

    let second = Section::with_layout_and_blocks(
        doc.sections[0].layout.clone(),
        vec![Block::Heading(
            1,
            attr,
            vec![Inline::Str("Scene Two".into())],
        )],
    );
    doc.sections.push(second);

    let back = round_trip(&doc);

    // The heading's automatic style must inherit from SceneHeading, so the
    // name has to appear somewhere in the resolved parent chain.
    let found = back.sections.iter().flat_map(|s| s.blocks.iter()).any(|b| {
        let (a, _) = match b {
            Block::Heading(_, a, i) => (a, i),
            _ => return false,
        };
        a.kv.iter().any(|(k, v)| {
            k == "style"
                && (v == "SceneHeading"
                    || back
                        .styles
                        .paragraph_styles
                        .get(&StyleId::new(v.as_str()))
                        .and_then(|p| p.parent.as_ref())
                        .is_some_and(|p| p.as_str() == "SceneHeading"))
        })
    });
    assert!(
        found,
        "a heading opening a later section must still resolve through \
         SceneHeading, not a name synthesised from its level"
    );
}

/// A table style's borders must survive ODT export.
///
/// ODF has no table-level border concept — like conditional-region shading, a
/// grid is represented per cell. The writer already resolved style→cell for
/// **shading** but passed each cell's raw direct borders straight through, so
/// a table styled *Table Grid* (all six edges set, no direct cell borders)
/// exported with no borders at all: measured before this fix, every cell came
/// back with four `None` edges while its background survived.
///
/// The six edges carry distinct widths so the assertions discriminate
/// *position* — an outer edge on the table boundary, the interior gridline
/// inside — rather than merely "some border came back".
#[test]
fn table_style_borders_resolve_into_per_cell_borders_on_odt_export() {
    use loki_doc_model::content::table::core::Table;
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_doc_model::style::table_borders::TableBorders;
    use loki_doc_model::style::table_style::{TableProps, TableStyle};
    use loki_primitives::color::DocumentColor;

    let edge = |w: f64| {
        Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(w),
            color: DocumentColor::from_hex("#000000").ok(),
            spacing: None,
        })
    };
    // top 1, right 2, bottom 3, left 4, inside_h 5, inside_v 6.
    let borders = TableBorders {
        top: edge(1.0),
        right: edge(2.0),
        bottom: edge(3.0),
        left: edge(4.0),
        inside_h: edge(5.0),
        inside_v: edge(6.0),
    };

    let mut doc = sample_doc();
    doc.styles.table_styles.insert(
        StyleId::new("TableGrid"),
        TableStyle {
            id: StyleId::new("TableGrid"),
            display_name: Some("Table Grid".into()),
            parent: None,
            table_props: TableProps {
                borders: Some(borders),
                ..Default::default()
            },
            conditional: Default::default(),
            extensions: Default::default(),
        },
    );

    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("TableGrid".to_string()));
    for body in table.bodies.iter_mut() {
        for (r, row) in body.body_rows.iter_mut().enumerate() {
            for (c, cell) in row.cells.iter_mut().enumerate() {
                cell.blocks = vec![Block::Para(vec![Inline::Str(format!("r{r}c{c}"))])];
            }
        }
    }
    doc.sections[0].blocks.push(Block::Table(Box::new(table)));

    let back = round_trip(&doc);
    let t = back
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table survives export");

    let rows: Vec<_> = t.bodies.iter().flat_map(|b| b.body_rows.iter()).collect();
    assert_eq!(rows.len(), 2, "both rows survive");

    let w = |b: &Option<Border>| b.as_ref().map(|x| x.width.value());

    // (0,0): top/left are the table's outer edges; bottom/right the interiors.
    let c00 = &rows[0].cells[0].props;
    assert_eq!(w(&c00.border_top), Some(1.0), "cell(0,0) top = outer top");
    assert_eq!(
        w(&c00.border_left),
        Some(4.0),
        "cell(0,0) left = outer left"
    );
    assert_eq!(
        w(&c00.border_bottom),
        Some(5.0),
        "cell(0,0) bottom = interior horizontal"
    );
    assert_eq!(
        w(&c00.border_right),
        Some(6.0),
        "cell(0,0) right = interior vertical"
    );

    // (1,1): the mirror — interiors above/left, outer edges below/right.
    let c11 = &rows[1].cells[1].props;
    assert_eq!(
        w(&c11.border_top),
        Some(5.0),
        "cell(1,1) top = interior horizontal"
    );
    assert_eq!(
        w(&c11.border_left),
        Some(6.0),
        "cell(1,1) left = interior vertical"
    );
    assert_eq!(
        w(&c11.border_bottom),
        Some(3.0),
        "cell(1,1) bottom = outer bottom"
    );
    assert_eq!(
        w(&c11.border_right),
        Some(2.0),
        "cell(1,1) right = outer right"
    );
}

/// The other half of the precedence rule: a direct cell border wins over the
/// style's edge, **per edge** — the cell keeps its own top and still takes the
/// remaining three from the style. An all-or-nothing fallback passes the test
/// above and fails this one.
#[test]
fn a_direct_cell_border_wins_per_edge_over_the_table_style() {
    use loki_doc_model::content::table::core::Table;
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_doc_model::style::table_borders::TableBorders;
    use loki_doc_model::style::table_style::{TableProps, TableStyle};
    use loki_primitives::color::DocumentColor;

    let edge = |w: f64| {
        Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(w),
            color: DocumentColor::from_hex("#000000").ok(),
            spacing: None,
        })
    };
    let mut doc = sample_doc();
    doc.styles.table_styles.insert(
        StyleId::new("TableGrid"),
        TableStyle {
            id: StyleId::new("TableGrid"),
            display_name: None,
            parent: None,
            table_props: TableProps {
                borders: Some(TableBorders {
                    top: edge(1.0),
                    right: edge(2.0),
                    bottom: edge(3.0),
                    left: edge(4.0),
                    inside_h: edge(5.0),
                    inside_v: edge(6.0),
                }),
                ..Default::default()
            },
            conditional: Default::default(),
            extensions: Default::default(),
        },
    );

    let mut table = Table::grid(1, 1);
    table.set_style_name(Some("TableGrid".to_string()));
    for body in table.bodies.iter_mut() {
        for row in body.body_rows.iter_mut() {
            for cell in row.cells.iter_mut() {
                cell.blocks = vec![Block::Para(vec![Inline::Str("only".into())])];
                // A direct top border only — the other three must come from
                // the style.
                cell.props.border_top = edge(9.0);
            }
        }
    }
    doc.sections[0].blocks.push(Block::Table(Box::new(table)));

    let back = round_trip(&doc);
    let t = back
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table survives export");
    let props = &t
        .bodies
        .iter()
        .flat_map(|b| b.body_rows.iter())
        .next()
        .expect("a row")
        .cells[0]
        .props;
    let w = |b: &Option<Border>| b.as_ref().map(|x| x.width.value());

    assert_eq!(
        w(&props.border_top),
        Some(9.0),
        "the cell's own top border wins over the style's"
    );
    assert_eq!(
        w(&props.border_bottom),
        Some(3.0),
        "the other edges still come from the style (per-edge, not all-or-nothing)"
    );
    assert_eq!(w(&props.border_left), Some(4.0));
    assert_eq!(w(&props.border_right), Some(2.0));
}

/// A table style that inherits its border set from a parent must still export a
/// grid.
///
/// The style→cell border baking originally looked the referenced style up flat,
/// so a style deriving its grid via `basedOn` (DOCX's *Table Grid* is `basedOn`
/// *Normal Table*; any user style derived from *Table Grid* holds no
/// `w:tblBorders` of its own) contributed nothing and the grid vanished.
#[test]
fn table_style_borders_inherited_from_a_parent_still_export() {
    use loki_doc_model::content::table::core::Table;
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_doc_model::style::table_borders::TableBorders;
    use loki_doc_model::style::table_style::{TableProps, TableStyle};

    let edge = |w: f64| {
        Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(w),
            color: None,
            spacing: None,
        })
    };
    let mut doc = sample_doc();
    // Parent holds the borders; child declares none and points at it.
    doc.styles.table_styles.insert(
        StyleId::new("Base"),
        TableStyle {
            id: StyleId::new("Base"),
            display_name: None,
            parent: None,
            table_props: TableProps {
                borders: Some(TableBorders {
                    top: edge(1.0),
                    right: edge(2.0),
                    bottom: edge(3.0),
                    left: edge(4.0),
                    inside_h: edge(5.0),
                    inside_v: edge(6.0),
                }),
                ..Default::default()
            },
            conditional: Default::default(),
            extensions: Default::default(),
        },
    );
    doc.styles.table_styles.insert(
        StyleId::new("Child"),
        TableStyle {
            id: StyleId::new("Child"),
            display_name: None,
            parent: Some(StyleId::new("Base")),
            table_props: TableProps::default(),
            conditional: Default::default(),
            extensions: Default::default(),
        },
    );

    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("Child".to_string()));
    for body in table.bodies.iter_mut() {
        for row in body.body_rows.iter_mut() {
            for cell in row.cells.iter_mut() {
                cell.blocks = vec![Block::Para(vec![Inline::Str("x".into())])];
            }
        }
    }
    doc.sections[0].blocks.push(Block::Table(Box::new(table)));

    let back = round_trip(&doc);
    let t = back
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table survives");
    let w = |b: &Option<loki_doc_model::style::props::border::Border>| {
        b.as_ref().map(|x| x.width.value())
    };

    // Top-left cell: outer top (1) + outer left (4), interior right (6) and
    // interior bottom (5) — distinct widths, so the assertion pins position as
    // well as presence.
    let p = &t.bodies[0].body_rows[0].cells[0].props;
    assert_eq!(
        (
            w(&p.border_top),
            w(&p.border_right),
            w(&p.border_bottom),
            w(&p.border_left)
        ),
        (Some(1.0), Some(6.0), Some(5.0), Some(4.0)),
        "inherited border set must reach the cell"
    );
}

/// A table style's default cell padding (`w:tblCellMar`) must be baked into each
/// exported cell, since ODF has no table-level cell-margin concept.
#[test]
fn table_style_cell_padding_bakes_into_per_cell_padding_on_odt_export() {
    use loki_doc_model::content::table::core::Table;
    use loki_doc_model::style::table_padding::CellPadding;
    use loki_doc_model::style::table_style::{TableProps, TableStyle};

    let mut doc = sample_doc();
    doc.styles.table_styles.insert(
        StyleId::new("Padded"),
        TableStyle {
            id: StyleId::new("Padded"),
            display_name: None,
            parent: None,
            table_props: TableProps {
                // Four distinct values: a uniform inset would pass under any
                // permutation of the four `fo:padding-*` attributes.
                cell_padding: Some(CellPadding {
                    top: Some(Points::new(1.0)),
                    bottom: Some(Points::new(2.0)),
                    left: Some(Points::new(3.0)),
                    right: Some(Points::new(4.0)),
                }),
                ..Default::default()
            },
            conditional: Default::default(),
            extensions: Default::default(),
        },
    );

    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("Padded".to_string()));
    for body in table.bodies.iter_mut() {
        for row in body.body_rows.iter_mut() {
            for cell in row.cells.iter_mut() {
                cell.blocks = vec![Block::Para(vec![Inline::Str("x".into())])];
            }
        }
        // One cell overrides a single side directly, to pin that resolution is
        // per side rather than all-or-nothing.
        body.body_rows[1].cells[1].props.padding_left = Some(Points::new(9.0));
    }
    doc.sections[0].blocks.push(Block::Table(Box::new(table)));

    let back = round_trip(&doc);
    let t = back
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table survives");

    let plain = &t.bodies[0].body_rows[0].cells[0].props;
    assert_eq!(
        (
            plain.padding_top.map(|p| p.value()),
            plain.padding_bottom.map(|p| p.value()),
            plain.padding_left.map(|p| p.value()),
            plain.padding_right.map(|p| p.value())
        ),
        (Some(1.0), Some(2.0), Some(3.0), Some(4.0)),
        "a cell with no direct padding takes all four sides from the style"
    );

    let overridden = &t.bodies[0].body_rows[1].cells[1].props;
    assert_eq!(
        (
            overridden.padding_left.map(|p| p.value()),
            overridden.padding_right.map(|p| p.value()),
            overridden.padding_top.map(|p| p.value())
        ),
        (Some(9.0), Some(4.0), Some(1.0)),
        "a direct side wins only on that side; the rest still come from the style"
    );
}
