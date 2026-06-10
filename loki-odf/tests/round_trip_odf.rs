// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF conformance integration tests derived from the ODF standard and
//! [MS-OODF] specifications.
//!
//! Each test verifies a specific ODF spec requirement against a hand-crafted
//! minimal ODT archive built in memory.

mod helpers;

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::core::TableBody;
use loki_odf::odt::import::{OdtImportOptions, OdtImporter};

// ── Style inheritance ────────────────────────────────────────────────────────

/// ODF §2 styling inheritance: a child paragraph style that specifies only
/// `fo:font-weight="bold"` and inherits `fo:font-size="12pt"` from its parent
/// via `style:parent-style-name` must resolve to both properties through
/// `StyleCatalog::resolve_char`.
#[test]
fn odf4_style_inheritance_chain() {
    use loki_doc_model::style::catalog::StyleId;

    let styles = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"ParentStyle\" style:family=\"paragraph\">\
          <style:text-properties fo:font-size=\"12pt\"/>\
        </style:style>\
        <style:style style:name=\"ChildStyle\" style:family=\"paragraph\" \
          style:parent-style-name=\"ParentStyle\">\
          <style:text-properties fo:font-weight=\"bold\"/>\
        </style:style>\
      </office:styles>\
      <office:master-styles/>\
      </office:document-styles>";

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p text:style-name=\"ChildStyle\">Inherited paragraph.</text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    let catalog = &result.document.styles;

    let resolved = catalog
        .resolve_char(&StyleId::new("ChildStyle"))
        .expect("ChildStyle must be present in the catalog");

    // fo:font-size="12pt" comes from the parent via inheritance.
    let font_size = resolved.font_size.map(|p| p.value() as f32).unwrap_or(0.0);
    assert!(
        (font_size - 12.0).abs() < 0.5,
        "ChildStyle should inherit fo:font-size=12pt from ParentStyle, got {font_size}"
    );

    // fo:font-weight="bold" is defined directly on ChildStyle.
    assert_eq!(
        resolved.bold,
        Some(true),
        "ChildStyle must have bold=true from its direct fo:font-weight=bold"
    );
}

// ── Footnotes ────────────────────────────────────────────────────────────────

/// An ODF footnote (`text:note text:note-class="footnote"`) inside a body
/// paragraph is mapped to `Inline::Note(NoteKind::Footnote, _)` with at
/// least one body block.  ODF §6.3 (`text:note`).
#[test]
fn odf5_footnote_maps_to_inline_note() {
    use loki_doc_model::content::inline::{Inline, NoteKind};

    let styles = helpers::empty_styles_xml("1.2");

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p>Sentence with a footnote\
          <text:note text:id=\"fn1\" text:note-class=\"footnote\">\
            <text:note-citation>1</text:note-citation>\
            <text:note-body>\
              <text:p>Footnote body text.</text:p>\
            </text:note-body>\
          </text:note>\
          after the mark.\
        </text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF footnote should import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let has_footnote = all_blocks.iter().any(|b| {
        let inlines: &[Inline] = match b {
            Block::StyledPara(p) => &p.inlines,
            Block::Para(i) => i,
            _ => return false,
        };
        inlines
            .iter()
            .any(|i| matches!(i, Inline::Note(NoteKind::Footnote, blocks) if !blocks.is_empty()))
    });

    assert!(
        has_footnote,
        "at least one Inline::Note(Footnote) with non-empty body must be present"
    );
}

// ── Text fields in body ──────────────────────────────────────────────────────

/// A `text:page-number` field in the document body is mapped to
/// `Inline::Field` — the same code path used for page-number fields in
/// headers. ODF §6.7.
#[test]
fn odf6_body_page_number_field() {
    use loki_doc_model::content::inline::Inline;

    let styles = helpers::empty_styles_xml("1.2");

    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p>Page <text:page-number>1</text:page-number> of document.</text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let zip = helpers::build_odt_zip(content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("ODF page-number field should import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let has_field = all_blocks.iter().any(|b| {
        let inlines: &[Inline] = match b {
            Block::StyledPara(p) => &p.inlines,
            Block::Para(i) => i,
            _ => return false,
        };
        inlines.iter().any(|i| matches!(i, Inline::Field(_)))
    });

    assert!(
        has_field,
        "text:page-number in body must map to Inline::Field"
    );
}

// ── Table row-span ────────────────────────────────────────────────────────────

/// `table:number-rows-spanned="2"` on a cell must set `Cell.row_span = 2` on
/// the spanning cell. The `table:covered-table-cell` in the row below must be
/// filtered out so that the second row contains only the non-covered cell.
/// [ODF 1.3 §9.1.4]
#[test]
fn odf9_table_row_span_propagated() {
    // 2×2 table: top-left cell spans 2 rows; bottom-left is covered.
    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <table:table table:name=\"SpanTable\">\
          <table:table-column/>\
          <table:table-column/>\
          <table:table-row>\
            <table:table-cell table:number-rows-spanned=\"2\">\
              <text:p>Spanning cell</text:p>\
            </table:table-cell>\
            <table:table-cell>\
              <text:p>Row 1 Col 2</text:p>\
            </table:table-cell>\
          </table:table-row>\
          <table:table-row>\
            <table:covered-table-cell/>\
            <table:table-cell>\
              <text:p>Row 2 Col 2</text:p>\
            </table:table-cell>\
          </table:table-row>\
        </table:table>\
      </office:text></office:body>\
      </office:document-content>";

    let styles = helpers::empty_styles_xml("1.2");
    let zip = helpers::build_odt_zip(content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("table with row-span must import without error");

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
                Some(t)
            } else {
                None
            }
        })
        .expect("document must contain a table block");

    let body: &TableBody = table
        .bodies
        .first()
        .expect("table must have at least one body row group");

    assert_eq!(body.body_rows.len(), 2, "table must have two body rows");

    // Spanning cell: row_span must be 2.
    let spanning_cell = body.body_rows[0]
        .cells
        .first()
        .expect("first row must have cells");
    assert_eq!(
        spanning_cell.row_span, 2,
        "top-left cell must have row_span = 2"
    );

    // Second row: covered cell is suppressed, only the non-covered cell remains.
    assert_eq!(
        body.body_rows[1].cells.len(),
        1,
        "second row must contain exactly 1 cell (covered cell filtered out)"
    );
}

// ── Metadata ──────────────────────────────────────────────────────────────────

/// Assert that ODF metadata fields (title, creator, subject, last_modified_by)
/// are correctly parsed from meta.xml and mapped to the document model.
#[test]
fn odf_metadata_parsed() {
    let content = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p>Hello world.</text:p>\
      </office:text></office:body>\
      </office:document-content>";

    let styles = helpers::empty_styles_xml("1.2");

    let meta = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-meta \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
        xmlns:meta=\"urn:oasis:names:tc:opendocument:xmlns:meta:1.0\">\
      <office:meta>\
        <dc:title>Test Document</dc:title>\
        <dc:subject>Test Subject</dc:subject>\
        <meta:initial-creator>Test Author</meta:initial-creator>\
        <dc:creator>Last Editor</dc:creator>\
        <meta:creation-date>2026-01-01T00:00:00</meta:creation-date>\
      </office:meta>\
      </office:document-meta>";

    let zip = helpers::build_odt_zip(content, &styles, Some(meta));

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    assert_eq!(result.document.meta.title.as_deref(), Some("Test Document"));
    assert_eq!(result.document.meta.creator.as_deref(), Some("Test Author"));
    assert_eq!(
        result.document.meta.subject.as_deref(),
        Some("Test Subject")
    );
    assert_eq!(
        result.document.meta.last_modified_by.as_deref(),
        Some("Last Editor")
    );
}
