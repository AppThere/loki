// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration smoke tests: rich ODT fixture → import → assert document shape.

mod helpers;

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_odf::odt::import::{OdtImportOptions, OdtImporter};

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
    let all_blocks: Vec<&Block> = doc.sections.iter().flat_map(|s| s.blocks.iter()).collect();
    assert!(
        !all_blocks.is_empty(),
        "document must contain at least one block"
    );

    // ── 2. At least one Heading ─────────────────────────────────────────────
    let has_heading = all_blocks.iter().any(|b| matches!(b, Block::Heading(..)));
    assert!(has_heading, "at least one Block::Heading must be present");

    // ── 3. At least one BulletList ──────────────────────────────────────────
    let has_bullet_list = all_blocks
        .iter()
        .any(|b| matches!(b, Block::BulletList(..)));
    assert!(
        has_bullet_list,
        "at least one Block::BulletList must be present"
    );
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
        let style_val = attr
            .kv
            .iter()
            .find(|(k, _)| k == "style")
            .map(|(_, v)| v.as_str());
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

/// ODF-1 — paragraph border: `fo:border="1pt solid #000000"` on a paragraph
/// style reaches `ParaProps.border_top` (and all four sides).
#[test]
fn odf1_paragraph_border_mapped() {
    let content = helpers::para_props_content_xml();
    let styles = helpers::para_props_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    use loki_doc_model::style::catalog::StyleId;
    let catalog = &result.document.styles;
    let border_style = catalog
        .paragraph_styles
        .get(&StyleId::new("BorderPara"))
        .expect("catalog must contain 'BorderPara' paragraph style");

    assert!(
        border_style.para_props.border_top.is_some(),
        "BorderPara must have border_top set (ODF-1)"
    );
}

/// ODF-2 — tab stops: two tab stops on a paragraph style reach
/// `ParaProps.tab_stops` with `len >= 2`.
#[test]
fn odf2_paragraph_tab_stops_mapped() {
    let content = helpers::para_props_content_xml();
    let styles = helpers::para_props_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    use loki_doc_model::style::catalog::StyleId;
    let catalog = &result.document.styles;
    let tab_style = catalog
        .paragraph_styles
        .get(&StyleId::new("TabPara"))
        .expect("catalog must contain 'TabPara' paragraph style");

    let stops = tab_style
        .para_props
        .tab_stops
        .as_ref()
        .expect("TabPara must have tab_stops set (ODF-2)");
    assert!(
        stops.len() >= 2,
        "TabPara must have at least 2 tab stops, got {}",
        stops.len()
    );
}

/// ODF-3 — background colour: `fo:background-color="#FFFFCC"` on a paragraph
/// style reaches `ParaProps.background_color`.
#[test]
fn odf3_paragraph_background_color_mapped() {
    let content = helpers::para_props_content_xml();
    let styles = helpers::para_props_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    use loki_doc_model::style::catalog::StyleId;
    let catalog = &result.document.styles;
    let bg_style = catalog
        .paragraph_styles
        .get(&StyleId::new("BgPara"))
        .expect("catalog must contain 'BgPara' paragraph style");

    assert!(
        bg_style.para_props.background_color.is_some(),
        "BgPara must have background_color set (ODF-3)"
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

    let indent_pt = p1
        .para_props
        .indent_start
        .map(|p| p.value() as f32)
        .unwrap_or(0.0);
    // 1 cm = 28.346 pt
    assert!(
        (indent_pt - 28.346).abs() < PT_TOLERANCE,
        "P1 indent_start should be ~28.35 pt (1 cm), got {indent_pt}"
    );
}

// ── Header / footer gap coverage tests ───────────────────────────────────────

/// Default header and footer are populated from `style:header` /
/// `style:footer` on the active master page.
#[test]
fn hf_default_header_and_footer_populated() {
    let content = helpers::hf_content_xml();
    let styles = helpers::hf_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let layout = &result.document.sections[0].layout;

    let hdr = layout
        .header
        .as_ref()
        .expect("default header must be populated from style:header");
    assert!(
        !hdr.blocks.is_empty(),
        "default header must have at least one block"
    );

    let ftr = layout
        .footer
        .as_ref()
        .expect("default footer must be populated from style:footer");
    assert!(
        !ftr.blocks.is_empty(),
        "default footer must have at least one block"
    );
}

/// First-page header and footer are populated from `style:header-first` /
/// `style:footer-first` on the active master page.
#[test]
fn hf_first_page_variants_populated() {
    let content = helpers::hf_content_xml();
    let styles = helpers::hf_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let layout = &result.document.sections[0].layout;

    let hdr_first = layout
        .header_first
        .as_ref()
        .expect("first-page header must be populated from style:header-first");
    assert!(
        !hdr_first.blocks.is_empty(),
        "first-page header must have at least one block"
    );

    let ftr_first = layout
        .footer_first
        .as_ref()
        .expect("first-page footer must be populated from style:footer-first");
    assert!(
        !ftr_first.blocks.is_empty(),
        "first-page footer must have at least one block"
    );
}

/// Even-page header and footer are populated from `style:header-left` /
/// `style:footer-left` on the active master page.
#[test]
fn hf_even_page_variants_populated() {
    let content = helpers::hf_content_xml();
    let styles = helpers::hf_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let layout = &result.document.sections[0].layout;

    let hdr_even = layout
        .header_even
        .as_ref()
        .expect("even-page header must be populated from style:header-left");
    assert!(
        !hdr_even.blocks.is_empty(),
        "even-page header must have at least one block"
    );

    let ftr_even = layout
        .footer_even
        .as_ref()
        .expect("even-page footer must be populated from style:footer-left");
    assert!(
        !ftr_even.blocks.is_empty(),
        "even-page footer must have at least one block"
    );
}

/// The default header content contains a `text:page-number` field rendered
/// as `Inline::Field`, confirming the full inline parser is used (not the
/// old simplified flat-text collector).
#[test]
fn hf_header_contains_field_code() {
    use loki_doc_model::content::inline::Inline;

    let content = helpers::hf_content_xml();
    let styles = helpers::hf_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let layout = &result.document.sections[0].layout;
    let hdr = layout
        .header
        .as_ref()
        .expect("default header must be present");

    let has_field = hdr.blocks.iter().any(|b| {
        let inlines: &[Inline] = match b {
            Block::StyledPara(p) => &p.inlines,
            Block::Para(inlines) => inlines,
            _ => return false,
        };
        inlines.iter().any(|i| matches!(i, Inline::Field(_)))
    });
    assert!(
        has_field,
        "default header must contain Inline::Field from text:page-number"
    );
}

/// Layout integration: pages produced from an ODF document with headers/footers
/// have non-empty `header_items` and `footer_items`.
#[test]
fn hf_layout_pages_have_header_and_footer_items() {
    use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};

    let content = helpers::hf_content_xml();
    let styles = helpers::hf_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let mut resources = FontResources::default();
    let layout = layout_document(
        &mut resources,
        &result.document,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    );

    let loki_layout::DocumentLayout::Paginated(paginated) = layout else {
        panic!("expected paginated layout");
    };

    assert!(
        !paginated.pages.is_empty(),
        "layout must produce at least one page"
    );

    // Page 1 gets first-page header (header_first is set).
    let p1 = &paginated.pages[0];
    assert!(
        !p1.header_items.is_empty(),
        "page 1 must have header items (first-page variant)"
    );
    assert!(
        !p1.footer_items.is_empty(),
        "page 1 must have footer items (first-page variant)"
    );
}

/// Importing a document with two master pages produces two sections,
/// each with the correct page orientation and its own header.
#[test]
fn multi_master_page_creates_sections() {
    let content = helpers::multi_master_content_xml();
    let styles = helpers::multi_master_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("multi-master import should succeed");

    let doc = &result.document;

    // Two master pages → two sections.
    assert_eq!(
        doc.section_count(),
        2,
        "two master pages should produce two sections (got {})",
        doc.section_count()
    );

    // Section 0: portrait (width < height, ~A4 portrait).
    let s0 = doc.section_at(0).expect("section 0 must exist");
    let ps0 = &s0.layout.page_size;
    assert!(
        ps0.width.value() < ps0.height.value(),
        "section 0 should be portrait (width={:.1} height={:.1})",
        ps0.width.value(),
        ps0.height.value()
    );

    // Section 1: landscape (width > height, ~A4 landscape).
    let s1 = doc.section_at(1).expect("section 1 must exist");
    let ps1 = &s1.layout.page_size;
    assert!(
        ps1.width.value() > ps1.height.value(),
        "section 1 should be landscape (width={:.1} height={:.1})",
        ps1.width.value(),
        ps1.height.value()
    );

    // Each section has its own header from its master page.
    assert!(
        s0.layout.header.is_some(),
        "section 0 (portrait) should have a header"
    );
    assert!(
        s1.layout.header.is_some(),
        "section 1 (landscape) should have a header"
    );

    // Section 0 contains the portrait paragraph, section 1 the landscape one.
    assert_eq!(s0.blocks.len(), 1, "section 0 should have 1 block");
    assert_eq!(s1.blocks.len(), 1, "section 1 should have 1 block");
}

/// A document with a single master page still produces exactly one section
/// (no spurious section breaks when all paragraphs share the same master page).
#[test]
fn single_master_page_no_spurious_sections() {
    let content = helpers::heading_and_paragraphs_content_xml("1.2");
    let styles = helpers::rich_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    assert_eq!(
        result.document.section_count(),
        1,
        "single-master document must produce exactly one section"
    );
}

/// ODF cell properties (padding, vertical-align, background-color, border)
/// are parsed from `style:table-cell-properties` and mapped to `CellProps`.
#[test]
fn odf_cell_props_mapped() {
    use loki_doc_model::content::table::row::CellVerticalAlign;

    let content = helpers::cell_props_content_xml();
    let styles = helpers::cell_props_styles_xml();
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("cell-props fixture should import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let table = all_blocks
        .iter()
        .find_map(|b| {
            if let Block::Table(t) = b {
                Some(t.as_ref())
            } else {
                None
            }
        })
        .expect("table should be present");

    let row = &table.bodies[0].body_rows[0];
    assert_eq!(row.cells.len(), 2, "table should have 2 cells");

    // ── Cell 0: StyledCell ──────────────────────────────────────────────────
    // fo:padding="0.2cm" → all four edges ≈ 5.67pt
    let c0 = &row.cells[0];
    let pad_top = c0
        .props
        .padding_top
        .expect("cell 0 should have padding_top")
        .value() as f32;
    assert!(
        (pad_top - 5.67).abs() < PT_TOLERANCE,
        "padding_top should be ~5.67pt (0.2cm), got {pad_top:.3}"
    );
    let pad_bottom = c0
        .props
        .padding_bottom
        .expect("cell 0 should have padding_bottom")
        .value() as f32;
    assert!(
        (pad_bottom - 5.67).abs() < PT_TOLERANCE,
        "padding_bottom should be ~5.67pt (0.2cm), got {pad_bottom:.3}"
    );
    let pad_left = c0
        .props
        .padding_left
        .expect("cell 0 should have padding_left")
        .value() as f32;
    assert!(
        (pad_left - 5.67).abs() < PT_TOLERANCE,
        "padding_left should be ~5.67pt (0.2cm), got {pad_left:.3}"
    );
    assert_eq!(
        c0.props.vertical_align,
        Some(CellVerticalAlign::Middle),
        "style:vertical-align=\"middle\" should map to Middle"
    );
    assert!(
        c0.props.background_color.is_some(),
        "fo:background-color=\"#FFFF00\" should be mapped"
    );
    assert!(
        c0.props.border_top.is_some(),
        "fo:border shorthand should produce border_top"
    );
    assert!(
        c0.props.border_bottom.is_some(),
        "fo:border shorthand should produce border_bottom"
    );

    // ── Cell 1: BottomCell ──────────────────────────────────────────────────
    // fo:padding-top="0.1cm" ≈ 2.83pt, fo:padding-bottom="0.3cm" ≈ 8.50pt
    let c1 = &row.cells[1];
    let c1_pad_top = c1
        .props
        .padding_top
        .expect("cell 1 should have padding_top")
        .value() as f32;
    assert!(
        (c1_pad_top - 2.83).abs() < PT_TOLERANCE,
        "cell 1 padding_top should be ~2.83pt (0.1cm), got {c1_pad_top:.3}"
    );
    let c1_pad_bottom = c1
        .props
        .padding_bottom
        .expect("cell 1 should have padding_bottom")
        .value() as f32;
    assert!(
        (c1_pad_bottom - 8.50).abs() < PT_TOLERANCE,
        "cell 1 padding_bottom should be ~8.50pt (0.3cm), got {c1_pad_bottom:.3}"
    );
    assert_eq!(
        c1.props.vertical_align,
        Some(CellVerticalAlign::Bottom),
        "style:vertical-align=\"bottom\" should map to Bottom"
    );
    assert!(
        c1.props.background_color.is_none(),
        "cell 1 should have no background color"
    );
    assert!(
        c1.props.border_top.is_none(),
        "cell 1 should have no border"
    );
}
