// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration smoke tests: rich ODT fixture → import → assert document shape.

mod helpers;

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_odf::odt::import::{OdtImporter, OdtImportOptions};

// Tolerance for length conversions (cm → pt can have floating-point error).
const PT_TOLERANCE: f32 = 0.5;

/// Import a rich ODT fixture and validate the high-level document shape.
///
/// Checks:
/// 1. Document has at least one block.
/// 2. At least one block is a `Block::Heading`.
/// 3. At least one block is a `Block::BulletList`.
#[test]
fn import_rich_odt_smoke() {
    let content = helpers::rich_fixture_content_xml("1.2");
    let styles = helpers::empty_styles_xml("1.2");
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("rich ODT fixture should import without error");

    let doc = &result.document;

    // ── 1. Non-empty content ────────────────────────────────────────────────
    let all_blocks: Vec<&Block> =
        doc.sections.iter().flat_map(|s| s.blocks.iter()).collect();
    assert!(!all_blocks.is_empty(), "document must contain at least one block");

    // ── 2. At least one Heading ─────────────────────────────────────────────
    let has_heading = all_blocks.iter().any(|b| matches!(b, Block::Heading(..)));
    assert!(has_heading, "at least one Block::Heading must be present");

    // ── 3. At least one BulletList ──────────────────────────────────────────
    let has_bullet_list = all_blocks.iter().any(|b| matches!(b, Block::BulletList(..)));
    assert!(has_bullet_list, "at least one Block::BulletList must be present");
}

// ── Gap coverage tests ─────────────────────────────────────────────────────────

/// Gap #4 — page size: `fo:page-width` / `fo:page-height` parsed and mapped.
///
/// Uses `rich_styles_xml()` which declares an A4 page layout (21 cm × 29.7 cm)
/// via a `style:page-layout` element linked through a `style:master-page`.
#[test]
fn gap4_page_size_from_page_layout() {
    let content = helpers::rich_content_xml_with_styles();
    let styles = helpers::rich_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let section = &result.document.sections[0];
    let width_pt = section.layout.page_size.width.value() as f32;
    let height_pt = section.layout.page_size.height.value() as f32;

    // A4: 21 cm = ~595.28 pt, 29.7 cm = ~841.89 pt
    assert!(
        (width_pt - 595.28).abs() < PT_TOLERANCE,
        "page width should be ~595 pt (A4), got {width_pt}"
    );
    assert!(
        (height_pt - 841.89).abs() < PT_TOLERANCE,
        "page height should be ~842 pt (A4), got {height_pt}"
    );
}

/// Gap #1 — headings: ODF heading style properties resolved through catalog.
///
/// The fixture has `<text:h text:style-name="Heading_20_1">` and a style
/// `"Heading_20_1"` with 18 pt bold. Verifies that the ODF style name is
/// stored in `Block::Heading`'s NodeAttr so the flow engine can find it.
#[test]
fn gap1_heading_style_name_preserved_in_node_attr() {
    use loki_doc_model::content::attr::NodeAttr;

    let content = helpers::rich_content_xml_with_styles();
    let styles = helpers::rich_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let blocks = &result.document.sections[0].blocks;
    let heading = blocks.iter().find(|b| matches!(b, Block::Heading(..)));
    assert!(heading.is_some(), "Block::Heading must be present");

    if let Some(Block::Heading(level, attr, _)) = heading {
        assert_eq!(*level, 1, "heading level must be 1");
        let style_val = attr.kv.iter().find(|(k, _)| k == "style").map(|(_, v)| v.as_str());
        assert_eq!(
            style_val,
            Some("Heading_20_1"),
            "NodeAttr must carry style name 'Heading_20_1', got {:?}",
            style_val
        );
    }
}

/// Gap #1 — headings: style catalog has the heading style with correct props.
///
/// Verifies that the style `"Heading_20_1"` with 18 pt bold is in the catalog
/// so the layout engine can resolve it.
#[test]
fn gap1_heading_style_in_catalog() {
    use loki_doc_model::style::catalog::StyleId;

    let content = helpers::rich_content_xml_with_styles();
    let styles = helpers::rich_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let catalog = &result.document.styles;
    let heading_style = catalog
        .paragraph_styles
        .get(&StyleId::new("Heading_20_1"))
        .expect("catalog must contain 'Heading_20_1' paragraph style");

    let font_size_pt = heading_style
        .char_props
        .font_size
        .map(|p| p.value() as f32)
        .unwrap_or(0.0);
    assert!(
        (font_size_pt - 18.0).abs() < PT_TOLERANCE,
        "Heading_20_1 font_size should be 18 pt, got {font_size_pt}"
    );
    assert_eq!(
        heading_style.char_props.bold,
        Some(true),
        "Heading_20_1 must be bold"
    );
}

/// Gap #2 — fonts: `fo:font-family` reaches `CharProps.font_name`.
///
/// The `"BodyText"` style uses `fo:font-family="Liberation Serif"` (not
/// `style:font-name`). Verifies the fallback path is active.
#[test]
fn gap2_fo_font_family_mapped_to_char_props() {
    use loki_doc_model::style::catalog::StyleId;

    let content = helpers::rich_content_xml_with_styles();
    let styles = helpers::rich_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let catalog = &result.document.styles;
    let body_style = catalog
        .paragraph_styles
        .get(&StyleId::new("BodyText"))
        .expect("catalog must contain 'BodyText' paragraph style");

    assert_eq!(
        body_style.char_props.font_name.as_deref(),
        Some("Liberation Serif"),
        "fo:font-family must be mapped to CharProps.font_name"
    );
}

/// Gap #5 — indentation: `fo:margin-left` on automatic style reaches `ParaProps`.
///
/// The automatic style `"P1"` has `fo:margin-left="1cm"` ≈ 28.35 pt.
#[test]
fn gap5_fo_margin_left_mapped_to_para_props() {
    use loki_doc_model::style::catalog::StyleId;

    let content = helpers::rich_content_xml_with_styles();
    let styles = helpers::rich_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let catalog = &result.document.styles;
    let p1 = catalog
        .paragraph_styles
        .get(&StyleId::new("P1"))
        .expect("catalog must contain auto style 'P1'");

    let indent_pt = p1.para_props.indent_start.map(|p| p.value() as f32).unwrap_or(0.0);
    // 1 cm = 28.346 pt
    assert!(
        (indent_pt - 28.346).abs() < PT_TOLERANCE,
        "P1 indent_start should be ~28.35 pt (1 cm), got {indent_pt}"
    );
}
