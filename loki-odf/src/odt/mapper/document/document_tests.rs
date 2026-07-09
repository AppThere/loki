// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the document mapper (moved out of `document/mod.rs` to keep
//! each module under the file-size ceiling).

use super::*;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::list_style::NumberingScheme;

use super::meta::parse_datetime;
use super::page::resolve_master_page_name;
use crate::odt::model::document::{OdfBodyChild, OdfDocument};
use crate::odt::model::paragraph::{OdfParagraph, OdfParagraphChild};
use crate::odt::model::styles::{OdfGraphicWrap, OdfStylesheet};
use crate::version::OdfVersion;
use loki_doc_model::style::StyleId;

fn empty_stylesheet() -> OdfStylesheet {
    OdfStylesheet::default()
}

fn options() -> OdtImportOptions {
    OdtImportOptions::default()
}

fn empty_doc(children: Vec<OdfBodyChild>) -> OdfDocument {
    OdfDocument {
        version: OdfVersion::V1_2,
        version_was_absent: false,
        body_children: children,
    }
}

fn text_paragraph(text: &str, is_heading: bool, level: Option<u8>) -> OdfParagraph {
    OdfParagraph {
        style_name: None,
        outline_level: level,
        is_heading,
        children: vec![OdfParagraphChild::Text(text.into())],
        list_context: None,
    }
}

/// A `style:family="graphic"` style carrying a `style:wrap` value.
fn graphic_style(name: &str, wrap: &str) -> OdfStyle {
    OdfStyle {
        name: name.into(),
        display_name: None,
        family: crate::odt::model::styles::OdfStyleFamily::Graphic,
        parent_name: None,
        list_style_name: None,
        para_props: None,
        text_props: None,
        col_width: None,
        cell_props: None,
        graphic_wrap: Some(OdfGraphicWrap {
            wrap: Some(wrap.into()),
            run_through: None,
        }),
        is_automatic: true,
        master_page_name: None,
    }
}

#[test]
fn floating_image_frame_becomes_inline_float_with_size() {
    use crate::odt::model::frames::{OdfFrame, OdfFrameKind};
    use loki_doc_model::content::float::{FloatWrap, WrapSide};

    // A paragraph anchoring a 1in × 1in floating image frame with a
    // `style:wrap="right"` graphic style, followed by body text.
    let frame = OdfFrame {
        name: None,
        style_name: Some("FrFloat".into()),
        anchor_type: Some("paragraph".into()),
        width: Some("1in".into()),
        height: Some("1in".into()),
        x: None,
        y: None,
        kind: OdfFrameKind::Image {
            href: "Pictures/x.png".into(),
            media_type: Some("image/png".into()),
            title: None,
            desc: None,
        },
    };
    let para = OdfParagraph {
        style_name: None,
        outline_level: None,
        is_heading: false,
        children: vec![
            OdfParagraphChild::Frame(frame),
            OdfParagraphChild::Text("Body text beside the float.".into()),
        ],
        list_context: None,
    };
    let doc = empty_doc(vec![OdfBodyChild::Paragraph(para)]);

    let mut sheet = empty_stylesheet();
    sheet.auto_styles.push(graphic_style("FrFloat", "right"));
    let mut images = HashMap::new();
    images.insert(
        "Pictures/x.png".to_string(),
        ("image/png".to_string(), vec![0u8, 1, 2, 3]),
    );

    let (result, _) = map_document(&doc, &sheet, None, &images, &HashMap::new(), &options());

    // The float stays inline in its anchoring paragraph (no separate Figure).
    let blocks = &result.sections[0].blocks;
    assert_eq!(blocks.len(), 1, "float must not spill into a Figure block");
    let Block::StyledPara(p) = &blocks[0] else {
        panic!("expected StyledPara, got {:?}", blocks[0]);
    };
    let img_attr = p
        .inlines
        .iter()
        .find_map(|i| match i {
            Inline::Image(attr, _, _) => Some(attr),
            _ => None,
        })
        .expect("paragraph carries a floating image inline");

    // Size is carried as EMU (1in = 72pt = 914400 EMU).
    let cx = img_attr
        .kv
        .iter()
        .find(|(k, _)| k == "cx_emu")
        .map(|(_, v)| v.parse::<u64>().unwrap());
    assert_eq!(cx, Some(914_400), "1in width → 914400 EMU");

    // The wrap is readable and side=right (text on the right → float left).
    let wrap = FloatWrap::read(img_attr).expect("float wrap stored on the image");
    assert_eq!(wrap.side, WrapSide::Right);
    assert!(!wrap.behind_text);
}

#[test]
fn empty_document_produces_empty_section() {
    let doc = empty_doc(vec![]);
    let (result, warnings) = map_document(
        &doc,
        &empty_stylesheet(),
        None,
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );
    assert!(warnings.is_empty());
    assert_eq!(result.sections.len(), 1);
    assert!(result.sections[0].blocks.is_empty());
}

#[test]
fn heading_is_emitted_as_heading_block() {
    let para = text_paragraph("Title", true, Some(1));
    let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
    let (result, _) = map_document(
        &doc,
        &empty_stylesheet(),
        None,
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );
    let blocks = &result.sections[0].blocks;
    assert_eq!(blocks.len(), 1);
    assert!(
        matches!(blocks[0], Block::Heading(1, _, _)),
        "expected Heading(1), got {:?}",
        blocks[0]
    );
}

#[test]
fn heading_suppressed_when_emit_heading_blocks_false() {
    let para = text_paragraph("Title", true, Some(1));
    let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
    let opts = OdtImportOptions {
        emit_heading_blocks: false,
        ..options()
    };
    let (result, _) = map_document(
        &doc,
        &empty_stylesheet(),
        None,
        &HashMap::new(),
        &HashMap::new(),
        &opts,
    );
    let blocks = &result.sections[0].blocks;
    assert_eq!(blocks.len(), 1);
    assert!(
        matches!(blocks[0], Block::StyledPara(_)),
        "expected StyledPara, got {:?}",
        blocks[0]
    );
}

#[test]
fn paragraph_is_emitted_as_styled_para() {
    let para = text_paragraph("Hello", false, None);
    let doc = empty_doc(vec![OdfBodyChild::Paragraph(para)]);
    let (result, _) = map_document(
        &doc,
        &empty_stylesheet(),
        None,
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );
    let blocks = &result.sections[0].blocks;
    assert!(
        matches!(blocks[0], Block::StyledPara(_)),
        "expected StyledPara, got {:?}",
        blocks[0]
    );
}

#[test]
fn text_content_preserved_in_heading() {
    let para = text_paragraph("Introduction", true, Some(1));
    let doc = empty_doc(vec![OdfBodyChild::Heading(para)]);
    let (result, _) = map_document(
        &doc,
        &empty_stylesheet(),
        None,
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );
    if let Block::Heading(_, _, inlines) = &result.sections[0].blocks[0] {
        assert_eq!(inlines.len(), 1);
        assert!(matches!(&inlines[0], loki_doc_model::Inline::Str(s) if s == "Introduction"));
    } else {
        panic!("expected Heading");
    }
}

#[test]
fn meta_title_mapped() {
    let odf_meta = OdfMeta {
        title: Some("My Document".into()),
        subject: Some("My Subject".into()),
        creator: Some("Alice".into()),
        initial_creator: Some("Bob".into()),
        keywords: vec!["k1".into(), "k2".into()],
        ..Default::default()
    };
    let doc = empty_doc(vec![]);
    let (result, _) = map_document(
        &doc,
        &empty_stylesheet(),
        Some(&odf_meta),
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );
    assert_eq!(result.meta.title.as_deref(), Some("My Document"));
    assert_eq!(result.meta.subject.as_deref(), Some("My Subject"));
    assert_eq!(result.meta.last_modified_by.as_deref(), Some("Alice"));
    assert_eq!(result.meta.creator.as_deref(), Some("Bob"));
    assert_eq!(result.meta.keywords.as_deref(), Some("k1, k2"));
}

#[test]
fn parse_datetime_rfc3339() {
    let dt = parse_datetime("2024-06-15T12:30:00Z");
    assert!(dt.is_some());
}

#[test]
fn parse_datetime_no_tz() {
    let dt = parse_datetime("2024-06-15T12:30:00");
    assert!(dt.is_some());
}

#[test]
fn parse_datetime_invalid_returns_none() {
    let dt = parse_datetime("not-a-date");
    assert!(dt.is_none());
}

// ── resolve_master_page_name unit tests ───────────────────────────────────

fn style_with_mpn(name: &str, mpn: Option<&str>, parent: Option<&str>) -> OdfStyle {
    use crate::odt::model::styles::OdfStyleFamily;
    OdfStyle {
        name: name.into(),
        display_name: None,
        family: OdfStyleFamily::Paragraph,
        parent_name: parent.map(String::from),
        list_style_name: None,
        para_props: None,
        text_props: None,
        col_width: None,
        cell_props: None,
        graphic_wrap: None,
        is_automatic: false,
        master_page_name: mpn.map(String::from),
    }
}

fn make_lookup(styles: &[OdfStyle]) -> HashMap<&str, &OdfStyle> {
    styles.iter().map(|s| (s.name.as_str(), s)).collect()
}

/// Direct `master_page_name` on the style is returned.
#[test]
fn resolve_mpn_direct() {
    let styles = [style_with_mpn("LandscapeStyle", Some("Landscape"), None)];
    let lookup = make_lookup(&styles);
    assert_eq!(
        resolve_master_page_name("LandscapeStyle", &lookup),
        Some("Landscape".into())
    );
}

/// When the style has no `master_page_name` but its parent does, the
/// parent's value is returned.
#[test]
fn resolve_mpn_inherited_from_parent() {
    let styles = [
        style_with_mpn("Base", Some("Landscape"), None),
        style_with_mpn("Child", None, Some("Base")),
    ];
    let lookup = make_lookup(&styles);
    assert_eq!(
        resolve_master_page_name("Child", &lookup),
        Some("Landscape".into())
    );
}

/// An empty `master_page_name` string is treated as absent — `None` returned.
#[test]
fn resolve_mpn_empty_string_returns_none() {
    let styles = [style_with_mpn("PlainStyle", Some(""), None)];
    let lookup = make_lookup(&styles);
    assert_eq!(resolve_master_page_name("PlainStyle", &lookup), None);
}

/// A style with no master page anywhere in the chain returns `None`.
#[test]
fn resolve_mpn_no_master_page_in_chain() {
    let styles = [
        style_with_mpn("Root", None, None),
        style_with_mpn("Child", None, Some("Root")),
    ];
    let lookup = make_lookup(&styles);
    assert_eq!(resolve_master_page_name("Child", &lookup), None);
}

/// A style that doesn't exist in the lookup returns `None` without panicking.
#[test]
fn resolve_mpn_unknown_style_returns_none() {
    let styles: [OdfStyle; 0] = [];
    let lookup = make_lookup(&styles);
    assert_eq!(resolve_master_page_name("NonExistent", &lookup), None);
}

// ── Nested tables ─────────────────────────────────────────────────────────

use crate::odt::model::tables::{OdfTable, OdfTableCell, OdfTableRow};

/// A single-cell table whose only block content is `content`.
fn single_cell_table(content: Vec<OdfBodyChild>) -> OdfTable {
    OdfTable {
        name: None,
        style_name: None,
        col_defs: vec![],
        rows: vec![OdfTableRow {
            style_name: None,
            cells: vec![OdfTableCell {
                style_name: None,
                col_span: 1,
                row_span: 1,
                is_covered: false,
                value_type: None,
                content,
            }],
        }],
    }
}

fn body_para(text: &str) -> OdfBodyChild {
    OdfBodyChild::Paragraph(text_paragraph(text, false, None))
}

/// A `table:table` nested inside a `table:table-cell` is mapped to a
/// `Block::Table` *inside* the outer cell's blocks, in document order with
/// any sibling paragraphs.
#[test]
fn nested_table_in_cell_maps_to_inner_table_block() {
    let inner = single_cell_table(vec![body_para("Inner")]);
    let outer = single_cell_table(vec![body_para("Before"), OdfBodyChild::Table(inner)]);
    let doc = empty_doc(vec![OdfBodyChild::Table(outer)]);

    let (result, _) = map_document(
        &doc,
        &empty_stylesheet(),
        None,
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );

    let Block::Table(t) = &result.sections[0].blocks[0] else {
        panic!(
            "expected outer Block::Table, got {:?}",
            result.sections[0].blocks[0]
        );
    };
    let cell = &t.bodies[0].body_rows[0].cells[0];
    // The cell holds the leading paragraph followed by the nested table,
    // in order.
    assert_eq!(cell.blocks.len(), 2, "paragraph + nested table preserved");
    assert!(
        matches!(cell.blocks[0], Block::StyledPara(_)),
        "first block is the sibling paragraph, got {:?}",
        cell.blocks[0]
    );
    let Block::Table(nested) = &cell.blocks[1] else {
        panic!(
            "second block must be the nested table, got {:?}",
            cell.blocks[1]
        );
    };
    // The nested table's own cell carries its paragraph.
    assert_eq!(nested.bodies[0].body_rows[0].cells[0].blocks.len(), 1);
}

// ── Page-number format ────────────────────────────────────────────────────

use crate::odt::model::document::{OdfMasterPage, OdfPageLayout};

/// Build a stylesheet whose "Standard" master page references a page layout
/// with the given `style:num-format`.
fn stylesheet_with_page_num_format(num_format: Option<&str>) -> OdfStylesheet {
    let mut sheet = OdfStylesheet::default();
    sheet.page_layouts.push(OdfPageLayout {
        name: "PL1".into(),
        page_width: None,
        page_height: None,
        margin_top: None,
        margin_bottom: None,
        margin_left: None,
        margin_right: None,
        print_orientation: None,
        num_format: num_format.map(String::from),
        columns: None,
        header_props: None,
        footer_props: None,
    });
    sheet.master_pages.push(OdfMasterPage {
        name: "Standard".into(),
        display_name: None,
        page_layout_name: "PL1".into(),
        header: None,
        footer: None,
        header_first: None,
        footer_first: None,
        header_even: None,
        footer_even: None,
    });
    sheet
}

fn page_number_format_of(sheet: &OdfStylesheet) -> Option<NumberingScheme> {
    let doc = empty_doc(vec![]);
    let (result, _) = map_document(
        &doc,
        sheet,
        None,
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );
    result.sections[0].layout.page_number_format
}

/// `style:num-format="i"` on the active master page's layout sets the
/// section's page-number format to lower Roman.
#[test]
fn page_num_format_lower_roman_maps() {
    let sheet = stylesheet_with_page_num_format(Some("i"));
    assert_eq!(
        page_number_format_of(&sheet),
        Some(NumberingScheme::LowerRoman)
    );
}

/// `style:num-format="A"` maps to upper-letter page numbering.
#[test]
fn page_num_format_upper_alpha_maps() {
    let sheet = stylesheet_with_page_num_format(Some("A"));
    assert_eq!(
        page_number_format_of(&sheet),
        Some(NumberingScheme::UpperAlpha)
    );
}

/// Decimal (`"1"`) and an absent `style:num-format` both leave the format
/// unset (decimal is the renderer default — no need to carry it).
#[test]
fn page_num_format_decimal_and_absent_stay_none() {
    assert_eq!(
        page_number_format_of(&stylesheet_with_page_num_format(Some("1"))),
        None
    );
    assert_eq!(
        page_number_format_of(&stylesheet_with_page_num_format(None)),
        None
    );
}

// ── Master-page transitions (5.7) ─────────────────────────────────────────

/// A portrait `PLstd` + landscape `PLland` page layout, a `Standard` and a
/// `Landscape` master page referencing them, and a `LandscapeStyle` paragraph
/// style whose `style:master-page-name` points at `Landscape`.
fn two_master_stylesheet() -> OdfStylesheet {
    let layout = |name: &str, w: &str, h: &str| OdfPageLayout {
        name: name.into(),
        page_width: Some(w.into()),
        page_height: Some(h.into()),
        margin_top: None,
        margin_bottom: None,
        margin_left: None,
        margin_right: None,
        print_orientation: None,
        num_format: None,
        columns: None,
        header_props: None,
        footer_props: None,
    };
    let master = |name: &str, pl: &str| OdfMasterPage {
        name: name.into(),
        display_name: None,
        page_layout_name: pl.into(),
        header: None,
        footer: None,
        header_first: None,
        footer_first: None,
        header_even: None,
        footer_even: None,
    };
    let mut sheet = OdfStylesheet::default();
    sheet.page_layouts.push(layout("PLstd", "8.5in", "11in"));
    sheet.page_layouts.push(layout("PLland", "11in", "8.5in"));
    sheet.master_pages.push(master("Standard", "PLstd"));
    sheet.master_pages.push(master("Landscape", "PLland"));
    sheet
        .named_styles
        .push(style_with_mpn("LandscapeStyle", Some("Landscape"), None));
    sheet
}

/// A paragraph whose `style:master-page-name` (via its style) differs from the
/// running master page starts a **new section** on a **new page**, with the new
/// master's geometry and the new page-style reference — the ODF equivalent of a
/// Word section break.
#[test]
fn master_page_transition_splits_into_sections() {
    let sheet = two_master_stylesheet();
    let first = OdfBodyChild::Paragraph(text_paragraph("On Standard", false, None));
    let mut second_para = text_paragraph("On Landscape", false, None);
    second_para.style_name = Some("LandscapeStyle".into());
    let doc = empty_doc(vec![first, OdfBodyChild::Paragraph(second_para)]);

    let (result, _) = map_document(
        &doc,
        &sheet,
        None,
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );

    assert_eq!(
        result.sections.len(),
        2,
        "the master-page change must split the body into two sections"
    );
    // Section 0: the running "Standard" master (portrait), page-style "Standard".
    assert_eq!(
        result.sections[0].page_style.as_ref().map(StyleId::as_str),
        Some("Standard")
    );
    let s0 = &result.sections[0].layout.page_size;
    assert!(
        s0.width.value() < s0.height.value(),
        "Standard section is portrait: {s0:?}"
    );
    // Section 1: the transitioned "Landscape" master (landscape), new page.
    assert_eq!(
        result.sections[1].page_style.as_ref().map(StyleId::as_str),
        Some("Landscape")
    );
    let s1 = &result.sections[1].layout.page_size;
    assert!(
        s1.width.value() > s1.height.value(),
        "Landscape section is landscape: {s1:?}"
    );
    assert_eq!(
        result.sections[1].start,
        loki_doc_model::layout::section::SectionStart::NewPage,
        "an ODF master-page transition is always a page break"
    );
    // The catalog registered both master pages as named page styles.
    assert!(
        result
            .styles
            .page_styles
            .contains_key(&StyleId::new("Standard"))
    );
    assert!(
        result
            .styles
            .page_styles
            .contains_key(&StyleId::new("Landscape"))
    );
}

/// A document whose **first** paragraph already declares a master page
/// different from the document default must NOT produce a spurious empty
/// leading section — the first paragraph simply sets the opening master page.
#[test]
fn leading_master_page_declaration_does_not_emit_empty_section() {
    let sheet = two_master_stylesheet();
    let mut first = text_paragraph("Starts on Landscape", false, None);
    first.style_name = Some("LandscapeStyle".into());
    let doc = empty_doc(vec![OdfBodyChild::Paragraph(first)]);

    let (result, _) = map_document(
        &doc,
        &sheet,
        None,
        &HashMap::new(),
        &HashMap::new(),
        &options(),
    );

    assert_eq!(
        result.sections.len(),
        1,
        "a leading master declaration must not create an empty preceding section"
    );
    assert_eq!(
        result.sections[0].page_style.as_ref().map(StyleId::as_str),
        Some("Landscape"),
        "the single section adopts the declared master page"
    );
}
