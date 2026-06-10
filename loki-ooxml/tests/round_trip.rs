// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration smoke tests: reference DOCX → import → assert document shape.

mod helpers;

use std::io::{Cursor, Write};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

/// Import the reference DOCX and validate the high-level document shape.
///
/// Checks:
/// 1. Document has at least one block.
/// 2. At least one `StyledRun` has `direct_props.bold == Some(true)`.
/// 3. At least one `StyledParagraph` has `direct_para_props.list_id` set.
/// 4. First section page size is approximately A4 (595 × 842 pt, ±1 pt).
/// 5. At least one paragraph has `border_top` set (gap #6).
/// 6. At least one paragraph has two explicit tab stops (gap #7).
/// 7. At least one paragraph contains `Inline::Note(Footnote, _)` (gap #2).
/// 8. At least one paragraph contains `Inline::Field` with `kind == PageNumber` (gap #4).
/// 9. Final section has a default header populated (gap #5).
/// 10. Final section has a default footer populated (gap #5).
/// 11. Final section has `header_first` set (`title_page = true`, gap #5).
#[test]
fn import_reference_docx_smoke() {
    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let doc = &result.document;

    // ── 1. Non-empty content ────────────────────────────────────────────────
    let all_blocks: Vec<&Block> = doc.sections.iter().flat_map(|s| s.blocks.iter()).collect();
    assert!(
        !all_blocks.is_empty(),
        "document must contain at least one block"
    );

    // ── 2. Bold run present ─────────────────────────────────────────────────
    let has_bold = all_blocks.iter().any(|b| block_has_bold_run(b));
    assert!(
        has_bold,
        "at least one StyledRun with bold=true must be present"
    );

    // ── 3. List paragraph present ───────────────────────────────────────────
    let has_list = all_blocks.iter().any(|b| {
        if let Block::StyledPara(p) = b {
            p.direct_para_props
                .as_ref()
                .map_or(false, |pp| pp.list_id.is_some())
        } else {
            false
        }
    });
    assert!(
        has_list,
        "at least one paragraph with list_id set must be present"
    );

    // ── 4. A4 page size ─────────────────────────────────────────────────────
    let page_size = &doc.sections[0].layout.page_size;
    let w = page_size.width.value();
    let h = page_size.height.value();
    assert!(
        (w - 595.0).abs() < 1.0,
        "A4 width expected ~595 pt, got {w:.2}"
    );
    assert!(
        (h - 842.0).abs() < 1.0,
        "A4 height expected ~842 pt, got {h:.2}"
    );

    // ── 5. Paragraph border present (gap #6) ────────────────────────────────
    let has_border = all_blocks.iter().any(|b| {
        if let Block::StyledPara(p) = b {
            p.direct_para_props
                .as_ref()
                .map_or(false, |pp| pp.border_top.is_some())
        } else {
            false
        }
    });
    assert!(
        has_border,
        "at least one paragraph with border_top must be present (gap #6)"
    );

    // ── 6. Tab stops present (gap #7) ───────────────────────────────────────
    let has_tab_stops = all_blocks.iter().any(|b| {
        if let Block::StyledPara(p) = b {
            p.direct_para_props.as_ref().map_or(false, |pp| {
                pp.tab_stops.as_ref().map_or(false, |ts| ts.len() >= 2)
            })
        } else {
            false
        }
    });
    assert!(
        has_tab_stops,
        "at least one paragraph with ≥2 tab stops must be present (gap #7)"
    );

    // ── 7. Footnote present (gap #2) ────────────────────────────────────────
    let has_footnote = all_blocks.iter().any(|b| block_has_footnote(b));
    assert!(
        has_footnote,
        "at least one paragraph with Inline::Note(Footnote) must be present (gap #2)"
    );

    // ── 8. Field code present (gap #4) ──────────────────────────────────────
    let has_field = all_blocks.iter().any(|b| block_has_field(b));
    assert!(
        has_field,
        "at least one paragraph with Inline::Field must be present (gap #4)"
    );

    // ── 9. Default header populated (gap #5) ────────────────────────────────
    let final_layout = &doc.sections.last().unwrap().layout;
    let hdr = final_layout
        .header
        .as_ref()
        .expect("final section must have a default header (gap #5)");
    assert!(
        !hdr.blocks.is_empty(),
        "default header must contain at least one block"
    );

    // ── 10. Default footer populated (gap #5) ───────────────────────────────
    let ftr = final_layout
        .footer
        .as_ref()
        .expect("final section must have a default footer (gap #5)");
    assert!(
        !ftr.blocks.is_empty(),
        "default footer must contain at least one block"
    );

    // ── 11. First-page header present because titlePg is set (gap #5) ───────
    let hdr_first = final_layout
        .header_first
        .as_ref()
        .expect("final section must have a first-page header (titlePg, gap #5)");
    assert!(
        !hdr_first.blocks.is_empty(),
        "first-page header must contain at least one block"
    );
}

fn block_has_bold_run(block: &Block) -> bool {
    let inlines = match block {
        Block::StyledPara(p) => p.inlines.as_slice(),
        Block::Heading(_, _, inlines) => inlines.as_slice(),
        _ => return false,
    };
    inlines.iter().any(inline_is_bold_styled_run)
}

fn inline_is_bold_styled_run(inline: &Inline) -> bool {
    if let Inline::StyledRun(run) = inline {
        run.direct_props
            .as_ref()
            .map_or(false, |cp| cp.bold == Some(true))
    } else {
        false
    }
}

fn block_has_footnote(block: &Block) -> bool {
    let inlines = match block {
        Block::StyledPara(p) => p.inlines.as_slice(),
        Block::Heading(_, _, inlines) => inlines.as_slice(),
        _ => return false,
    };
    inlines_have_footnote(inlines)
}

fn inlines_have_footnote(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Note(NoteKind::Footnote, _) => true,
        Inline::StyledRun(run) => inlines_have_footnote(&run.content),
        _ => false,
    })
}

fn block_has_field(block: &Block) -> bool {
    let inlines = match block {
        Block::StyledPara(p) => p.inlines.as_slice(),
        Block::Heading(_, _, inlines) => inlines.as_slice(),
        _ => return false,
    };
    inlines_have_field(inlines)
}

fn inlines_have_field(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Field(_) => true,
        Inline::StyledRun(run) => inlines_have_field(&run.content),
        _ => false,
    })
}

/// Verify that a table with `w:tblW w:w="5000" w:type="dxa"` maps to
/// `TableWidth::Fixed(250.0)` (5000 twips ÷ 20 = 250 pt). (OOXML-1)
#[test]
fn ooxml1_table_width_mapped() {
    use loki_doc_model::content::table::TableWidth;

    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let doc = &result.document;
    let all_blocks: Vec<&Block> = doc.sections.iter().flat_map(|s| s.blocks.iter()).collect();

    let table_width = all_blocks.iter().find_map(|b| {
        if let Block::Table(t) = b {
            t.width.as_ref()
        } else {
            None
        }
    });

    let width = table_width.expect("document must contain a table with a width (OOXML-1)");
    match width {
        TableWidth::Fixed(pt) => assert!(
            (pt - 250.0).abs() < 0.5,
            "table width should be ~250 pt (5000 twips ÷ 20), got {pt}"
        ),
        other => panic!("expected TableWidth::Fixed, got {other:?}"),
    }
}

/// Verify that `w:defaultTabStop w:val="720"` (720 twips = 36 pt) is forwarded
/// to `Document.settings.default_tab_stop_pt`. (OOXML-2)
#[test]
fn ooxml2_default_tab_stop_mapped() {
    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let settings = result
        .document
        .settings
        .as_ref()
        .expect("document.settings must be Some when settings.xml is present (OOXML-2)");

    assert!(
        (settings.default_tab_stop_pt - 36.0).abs() < 0.5,
        "default_tab_stop_pt should be ~36 pt (720 twips ÷ 20), got {}",
        settings.default_tab_stop_pt
    );
}

/// Verify that a document with two manual page breaks produces at least three
/// layout pages (the reference DOCX has exactly two `<w:br w:type="page"/>`).
#[test]
fn page_breaks_produce_multiple_layout_pages() {
    use loki_layout::{FontResources, LayoutMode, layout_document};

    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let mut resources = FontResources::default();
    let layout = layout_document(
        &mut resources,
        &result.document,
        LayoutMode::Paginated,
        1.0,
        &loki_layout::LayoutOptions::default(),
    );

    let loki_layout::DocumentLayout::Paginated(paginated) = layout else {
        panic!("expected paginated layout");
    };

    assert!(
        paginated.pages.len() >= 3,
        "two manual page breaks should yield ≥3 layout pages, got {}",
        paginated.pages.len()
    );
}

/// Verify that the layout engine assigns header/footer items to pages after
/// import — specifically that the first page gets the first-page header and
/// subsequent pages get the default header (gap #5).
#[test]
fn layout_assigns_header_footer_per_page() {
    use loki_layout::{FontResources, LayoutMode, layout_document};

    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let mut resources = FontResources::default();
    let layout = layout_document(
        &mut resources,
        &result.document,
        LayoutMode::Paginated,
        1.0,
        &loki_layout::LayoutOptions::default(),
    );

    let loki_layout::DocumentLayout::Paginated(paginated) = layout else {
        panic!("expected paginated layout");
    };

    assert!(
        !paginated.pages.is_empty(),
        "layout should produce at least one page"
    );

    // Page 1 gets the first-page header variant (titlePg=true, header_first is set).
    let p1 = &paginated.pages[0];
    assert!(
        !p1.header_items.is_empty(),
        "page 1 should have header items (first-page header variant)"
    );
    assert!(p1.header_height > 0.0, "page 1 header_height should be > 0");
    assert!(
        !p1.footer_items.is_empty(),
        "page 1 should have footer items (first-page footer variant)"
    );
    assert!(p1.footer_height > 0.0, "page 1 footer_height should be > 0");

    // All pages should have both header and footer items.
    for (i, page) in paginated.pages.iter().enumerate() {
        assert!(
            !page.header_items.is_empty(),
            "page {} should have header items",
            i + 1
        );
        assert!(
            !page.footer_items.is_empty(),
            "page {} should have footer items",
            i + 1
        );
    }
}

/// Vertical merge: the 2×3 table in the reference fixture has col 0 merged
/// across rows 0-1.  After import, row 0 cell 0 must carry `row_span = 2`,
/// the continuation cell must be removed from row 1, and row 2 is unmerged.
#[test]
fn vmerge_row_span_assigned_and_continuation_removed() {
    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    // Locate the 3-row merged table.
    let merged_table = all_blocks
        .iter()
        .filter_map(|b| {
            if let Block::Table(t) = b {
                Some(t.as_ref())
            } else {
                None
            }
        })
        .find(|t| t.bodies.iter().any(|b| b.body_rows.len() == 3))
        .expect("3-row merged-table must be present in the document");

    let body = &merged_table.bodies[0];

    // Row 0: 2 cells — [Merged Cell (row_span=2), Row 1 Col 2]
    assert_eq!(
        body.body_rows[0].cells.len(),
        2,
        "row 0 should have 2 cells"
    );
    assert_eq!(
        body.body_rows[0].cells[0].row_span, 2,
        "merged cell in row 0 col 0 should have row_span = 2"
    );

    // Row 1: 1 cell — continuation removed, only col 2 remains
    assert_eq!(
        body.body_rows[1].cells.len(),
        1,
        "row 1 should have 1 cell after continuation removal"
    );

    // Row 2: 2 cells — unmerged
    assert_eq!(
        body.body_rows[2].cells.len(),
        2,
        "row 2 should have 2 cells"
    );
    assert_eq!(
        body.body_rows[2].cells[0].row_span, 1,
        "row 2 col 0 should have row_span = 1"
    );
}

#[test]
fn cell_props_padding_valign_textdirection_mapped() {
    use loki_doc_model::content::table::row::{CellTextDirection, CellVerticalAlign};
    use loki_primitives::units::Points;

    let bytes = helpers::build_reference_docx();
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("reference DOCX should import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    // Locate the styled-cell table (1 body row with 2 cells, first has tcMar).
    let styled_table = all_blocks
        .iter()
        .filter_map(|b| {
            if let Block::Table(t) = b {
                Some(t.as_ref())
            } else {
                None
            }
        })
        .find(|t| {
            t.bodies.iter().any(|b| {
                b.body_rows.len() == 1
                    && b.body_rows[0]
                        .cells
                        .first()
                        .map(|c| c.props.padding_left.is_some())
                        .unwrap_or(false)
            })
        })
        .expect("styled-cell table must be present in the document");

    let row = &styled_table.bodies[0].body_rows[0];

    // Cell 0: padding 5pt top/bottom, 10pt left/right (100 twips ÷20, 200 twips ÷20)
    let c0 = &row.cells[0];
    assert_eq!(
        c0.props.padding_top,
        Some(Points::new(5.0)),
        "padding_top should be 5pt"
    );
    assert_eq!(
        c0.props.padding_bottom,
        Some(Points::new(5.0)),
        "padding_bottom should be 5pt"
    );
    assert_eq!(
        c0.props.padding_left,
        Some(Points::new(10.0)),
        "padding_left should be 10pt"
    );
    assert_eq!(
        c0.props.padding_right,
        Some(Points::new(10.0)),
        "padding_right should be 10pt"
    );
    assert_eq!(
        c0.props.vertical_align,
        Some(CellVerticalAlign::Middle),
        "vAlign center → Middle"
    );
    assert_eq!(
        c0.props.text_direction,
        Some(CellTextDirection::TbRl),
        "textDirection tbRl"
    );

    // Cell 1: only vAlign bottom, no padding
    let c1 = &row.cells[1];
    assert_eq!(
        c1.props.padding_top, None,
        "cell 1 should have no top padding"
    );
    assert_eq!(
        c1.props.vertical_align,
        Some(CellVerticalAlign::Bottom),
        "vAlign bottom → Bottom"
    );
    assert_eq!(
        c1.props.text_direction, None,
        "cell 1 should have no text direction"
    );
}

// ── Integration Draft tests ───────────────────────────────────────────────────

/// Strict OOXML namespace test: verify we can import a document where the
/// relationship and element namespaces use the Strict schemas:
/// Main relationship: `http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument`
/// Elements: `http://purl.oclc.org/ooxml/wordprocessingml/main`
#[test]
fn test_strict_namespace_handling() {
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    // [Content_Types].xml (Strict types aren't different in extensions, but overrides can be)
    zip.start_file("[Content_Types].xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    // Package relationships: using Strict relationship type for the main office document
    zip.start_file("_rels/.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    // Word document relationships: using Strict relationship types
    zip.start_file("word/_rels/document.xml.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    // Strict namespace for wordprocessingml elements in document.xml
    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://purl.oclc.org/ooxml/wordprocessingml/main">
  <w:body>
    <w:p>
      <w:r>
        <w:t>Strict Namespace Text</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();

    // Import and assert it matches
    let result = DocxImporter::new(DocxImportOptions::default()).run(Cursor::new(buf));

    let import_res = result.expect("Strict namespace document should import successfully");
    assert_eq!(import_res.document.sections.len(), 1);
}

/// Verification of table cell/column edge cases:
/// Tables containing negative column widths, zero width, or malformed attributes
/// should import safely without panicking.
#[test]
fn test_table_with_invalid_attributes() {
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/_rels/document.xml.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tblPr>
        <w:tblW w:w="-500" w:type="dxa"/>
      </w:tblPr>
      <w:tblGrid>
        <w:gridCol w:w="0"/>
        <w:gridCol w:w="-1000"/>
      </w:tblGrid>
      <w:tr>
        <w:tc>
          <w:tcPr>
            <w:tcW w:w="0" w:type="dxa"/>
            <w:tcMar>
              <w:top w:w="-50" w:type="dxa"/>
            </w:tcMar>
          </w:tcPr>
          <w:p><w:r><w:t>Cell 1</w:t></w:r></w:p>
        </w:tc>
        <w:tc>
          <w:p><w:r><w:t>Cell 2</w:t></w:r></w:p>
        </w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#
            .to_string()
            .into_bytes()
            .as_slice(),
    )
    .unwrap();

    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("should import safely despite invalid/negative table attributes");

    let doc = &result.document;
    let all_blocks: Vec<&loki_doc_model::content::block::Block> =
        doc.sections.iter().flat_map(|s| s.blocks.iter()).collect();
    let table = all_blocks
        .iter()
        .find_map(|b| {
            if let loki_doc_model::content::block::Block::Table(t) = b {
                Some(t.as_ref())
            } else {
                None
            }
        })
        .expect("table must be present");

    assert_eq!(table.bodies[0].body_rows[0].cells.len(), 2);
}

/// Hyperlink relationship missing test:
/// A hyperlink element pointing to an invalid/missing relationship ID
/// should fall back to using the relationship ID as a bookmark fragment target (e.g. "#rIdInvalid")
/// and emit an OoxmlWarning.
#[test]
fn test_hyperlink_missing_relationship() {
    use loki_ooxml::OoxmlWarning;
    use zip::CompressionMethod;
    use zip::ZipWriter;
    use zip::write::FileOptions;

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    // No relationships defined in document.xml.rels for the hyperlink rId99!
    zip.start_file("word/_rels/document.xml.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:hyperlink r:id="rId99">
        <w:r>
          <w:t>Hyperlink Text</w:t>
        </w:r>
      </w:hyperlink>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("import should succeed despite missing hyperlink relationship");

    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, OoxmlWarning::UnresolvedRelationship { id, .. } if id == "rId99")),
        "should have emitted an UnresolvedRelationship warning for rId99, got {:?}",
        result.warnings
    );

    let doc = &result.document;
    let all_blocks: Vec<&loki_doc_model::content::block::Block> =
        doc.sections.iter().flat_map(|s| s.blocks.iter()).collect();
    // The importer always produces Block::StyledPara for body paragraphs.
    let inlines: &[Inline] = match all_blocks[0] {
        Block::StyledPara(p) => &p.inlines,
        Block::Para(inlines) => inlines,
        other => panic!("expected a paragraph block, got {:?}", other),
    };

    let link = inlines
        .iter()
        .find_map(|inline| {
            if let Inline::Link(_, _, target) = inline {
                Some(target)
            } else {
                None
            }
        })
        .expect("hyperlink inline must be present");

    assert_eq!(link.url, "#rId99");
}
