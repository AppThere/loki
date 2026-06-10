// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Security hardening tests: crafted hostile inputs must be clamped or
//! rejected with a typed error instead of exhausting memory or the stack.

use std::io::{Cursor, Write};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_odf::odt::import::{OdtImportOptions, OdtImporter};
use loki_odf::{OdfError, OdsImport, OdsImportOptions};
use zip::CompressionMethod;
use zip::write::{FileOptions, ZipWriter};

const MANIFEST: &[u8] = b"<manifest:manifest \
    xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" \
    manifest:version=\"1.2\"/>";

/// Build a minimal ODF ZIP with the given mimetype and `content.xml`.
fn build_odf_zip(mimetype: &str, content_xml: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));

    let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(mimetype.as_bytes()).unwrap();

    let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("META-INF/manifest.xml", deflated).unwrap();
    zip.write_all(MANIFEST).unwrap();
    zip.start_file("content.xml", deflated).unwrap();
    zip.write_all(content_xml).unwrap();
    zip.start_file("styles.xml", deflated).unwrap();
    zip.write_all(b"<office:document-styles/>").unwrap();

    zip.finish().unwrap();
    buf
}

/// Wrap `body` in a minimal ODT `content.xml`.
fn odt_content_xml(body: &str) -> Vec<u8> {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
  office:version="1.3">
  <office:body><office:text>{body}</office:text></office:body>
</office:document-content>"#
    )
    .into_bytes()
}

fn import_odt(body: &str) -> Result<loki_doc_model::Document, OdfError> {
    let zip = build_odf_zip(
        "application/vnd.oasis.opendocument.text",
        &odt_content_xml(body),
    );
    OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .map(|r| r.document)
}

// ── S4: recursion depth limits ────────────────────────────────────────────────

#[test]
fn deeply_nested_spans_are_rejected() {
    let mut body = String::from("<text:p>");
    for _ in 0..150 {
        body.push_str("<text:span>");
    }
    body.push('x');
    for _ in 0..150 {
        body.push_str("</text:span>");
    }
    body.push_str("</text:p>");

    let result = import_odt(&body);
    assert!(
        matches!(result, Err(OdfError::NestingTooDeep { limit: 100 })),
        "expected NestingTooDeep, got {result:?}"
    );
}

#[test]
fn deeply_nested_lists_are_rejected() {
    let mut body = String::new();
    for _ in 0..150 {
        body.push_str("<text:list><text:list-item>");
    }
    body.push_str("<text:p>x</text:p>");
    for _ in 0..150 {
        body.push_str("</text:list-item></text:list>");
    }

    let result = import_odt(&body);
    assert!(
        matches!(result, Err(OdfError::NestingTooDeep { limit: 100 })),
        "expected NestingTooDeep, got {result:?}"
    );
}

#[test]
fn moderately_nested_spans_still_import() {
    let mut body = String::from("<text:p>");
    for _ in 0..20 {
        body.push_str("<text:span>");
    }
    body.push('x');
    for _ in 0..20 {
        body.push_str("</text:span>");
    }
    body.push_str("</text:p>");
    assert!(import_odt(&body).is_ok());
}

// ── S3: allocation bombs ──────────────────────────────────────────────────────

#[test]
fn huge_space_count_is_clamped() {
    let doc = import_odt(r#"<text:p>A<text:s text:c="4000000000"/>B</text:p>"#).unwrap();
    let Block::StyledPara(para) = &doc.sections[0].blocks[0] else {
        panic!("expected StyledPara, got {:?}", doc.sections[0].blocks[0]);
    };
    let total_len: usize = para
        .inlines
        .iter()
        .map(|i| match i {
            Inline::Str(s) => s.len(),
            _ => 0,
        })
        .sum();
    // "A" + at most 10_000 clamped spaces + "B".
    assert!(
        total_len <= 10_002,
        "space run was not clamped: {total_len} bytes"
    );
    assert!(total_len >= 10_000, "clamped run unexpectedly short");
}

#[test]
fn huge_column_repeat_is_clamped() {
    let body = r#"<table:table>
        <table:table-column table:number-columns-repeated="1000000000"/>
        <table:table-row><table:table-cell><text:p>x</text:p></table:table-cell></table:table-row>
    </table:table>"#;
    let doc = import_odt(body).unwrap();
    let Block::Table(table) = &doc.sections[0].blocks[0] else {
        panic!("expected Table, got {:?}", doc.sections[0].blocks[0]);
    };
    assert_eq!(table.col_specs.len(), 16_384);
}

// ── S2: ODS repeat-count amplification ────────────────────────────────────────

#[test]
fn ods_huge_repeats_are_clamped_but_positions_advance() {
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
  office:version="1.3">
  <office:body><office:spreadsheet>
    <table:table table:name="Sheet1">
      <table:table-row>
        <table:table-cell office:value-type="string" table:number-columns-repeated="4000000000"><text:p>X</text:p></table:table-cell>
        <table:table-cell office:value-type="string"><text:p>Y</text:p></table:table-cell>
      </table:table-row>
      <table:table-row table:number-rows-repeated="4000000000">
        <table:table-cell/>
      </table:table-row>
      <table:table-row>
        <table:table-cell office:value-type="string"><text:p>Z</text:p></table:table-cell>
      </table:table-row>
    </table:table>
  </office:spreadsheet></office:body>
</office:document-content>"#;
    let zip = build_odf_zip(
        "application/vnd.oasis.opendocument.spreadsheet",
        content.as_bytes(),
    );

    let workbook = OdsImport::import(Cursor::new(zip), OdsImportOptions::default()).unwrap();
    let sheet = &workbook.sheets[0];

    // "X" is materialized at most MAX_MATERIALIZED_REPEAT (10_000) times,
    // plus one "Y" and one "Z".
    assert_eq!(sheet.cells.len(), 10_002, "materialization was not clamped");

    // The column cursor advanced by the full sheet-clamped repeat (16_384),
    // so "Y" lands immediately after the repeated range.
    assert_eq!(
        sheet.cells.get(&(0, 16_384)).map(|c| c.value.as_str()),
        Some("Y")
    );

    // The row cursor advanced by the full sheet-clamped repeat (1_048_576)
    // over the empty filler row, so "Z" lands at row 1 + 1_048_576.
    assert_eq!(
        sheet.cells.get(&(1_048_577, 0)).map(|c| c.value.as_str()),
        Some("Z")
    );
}

#[test]
fn ods_row_times_column_repeat_is_bounded_by_aggregate_budget() {
    // A single cell with both axes repeated near the per-axis cap would
    // expand to MAX_MATERIALIZED_REPEAT² = 10⁸ cells without the aggregate
    // budget. The whole-workbook cap (2_000_000) must bound the total.
    let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
  office:version="1.3">
  <office:body><office:spreadsheet>
    <table:table table:name="Sheet1">
      <table:table-row table:number-rows-repeated="100000">
        <table:table-cell office:value-type="string" table:number-columns-repeated="100000"><text:p>X</text:p></table:table-cell>
      </table:table-row>
    </table:table>
  </office:spreadsheet></office:body>
</office:document-content>"#;
    let zip = build_odf_zip(
        "application/vnd.oasis.opendocument.spreadsheet",
        content.as_bytes(),
    );

    let workbook = OdsImport::import(Cursor::new(zip), OdsImportOptions::default()).unwrap();
    let sheet = &workbook.sheets[0];

    assert!(
        sheet.cells.len() as u64 <= 2_000_000,
        "aggregate materialization budget exceeded: {} cells",
        sheet.cells.len()
    );
}
