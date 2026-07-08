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

/// A table's named-style reference (`table:style-name`) survives ODT export →
/// import: the writer emits the `<style:style style:family="table">` definition
/// in styles.xml and the reference on the table; import restores `style_name`.
#[test]
fn table_style_name_reference_round_trips() {
    use loki_doc_model::content::table::core::Table;
    use loki_doc_model::style::table_style::{TableProps, TableStyle, TableWidth};
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
}
