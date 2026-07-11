// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the `word/document.xml` reader, extracted from `document.rs` to
//! hold the 300-line ceiling.

use super::table::parse_tbl_look;
use super::*;

const SIMPLE_DOC: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/><w:outlineLvl w:val="0"/></w:pPr>
      <w:r><w:t>Hello</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t xml:space="preserve">World </w:t></w:r>
    </w:p>
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"
               w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#;

#[test]
fn parses_two_paragraphs() {
    let doc = parse_document(SIMPLE_DOC).unwrap();
    let paras: Vec<_> = doc
        .body
        .children
        .iter()
        .filter(|c| matches!(c, DocxBodyChild::Paragraph(_)))
        .collect();
    assert_eq!(paras.len(), 2);
}

#[test]
fn first_para_has_style() {
    let doc = parse_document(SIMPLE_DOC).unwrap();
    if let Some(DocxBodyChild::Paragraph(p)) = doc.body.children.first() {
        assert_eq!(
            p.ppr.as_ref().and_then(|ppr| ppr.style_id.as_deref()),
            Some("Heading1")
        );
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn final_sect_pr_parsed() {
    let doc = parse_document(SIMPLE_DOC).unwrap();
    let sect = doc.body.final_sect_pr.unwrap();
    let pg_sz = sect.pg_sz.unwrap();
    assert_eq!(pg_sz.w, 12240);
    assert_eq!(pg_sz.h, 15840);
}

#[test]
fn parse_tbl_look_reads_the_legacy_val_bitmask() {
    use quick_xml::events::BytesStart;
    // 0x04A0 = firstRow + firstColumn + noVBand (Word's default look).
    let e = BytesStart::new("w:tblLook").with_attributes([("w:val", "04A0")]);
    let look = parse_tbl_look(&e);
    assert!(look.first_row);
    assert!(look.first_column);
    assert!(look.h_band); // noHBand off → horizontal banding on
    assert!(!look.last_row);
    assert!(!look.last_column);
    assert!(!look.v_band); // noVBand set → vertical banding off
}

#[test]
fn parse_tbl_look_prefers_explicit_attributes() {
    use quick_xml::events::BytesStart;
    let e = BytesStart::new("w:tblLook").with_attributes([
        ("w:firstRow", "0"),
        ("w:lastRow", "1"),
        ("w:firstColumn", "0"),
        ("w:lastColumn", "1"),
        ("w:noHBand", "1"), // banding off
        ("w:noVBand", "0"), // banding on
    ]);
    let look = parse_tbl_look(&e);
    assert!(!look.first_row);
    assert!(look.last_row);
    assert!(!look.first_column);
    assert!(look.last_column);
    assert!(!look.h_band);
    assert!(look.v_band);
}

/// A block-level content control (`w:sdt`) wrapping two paragraphs, a nested
/// content control, and a table — the kind Word emits for cover pages/forms.
const SDT_DOC: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Before</w:t></w:r></w:p>
    <w:sdt>
      <w:sdtPr><w:alias w:val="Title"/><w:tag w:val="t"/><w:id w:val="1"/></w:sdtPr>
      <w:sdtContent>
        <w:p><w:r><w:t>Inside one</w:t></w:r></w:p>
        <w:sdt>
          <w:sdtPr><w:id w:val="2"/></w:sdtPr>
          <w:sdtContent><w:p><w:r><w:t>Nested</w:t></w:r></w:p></w:sdtContent>
        </w:sdt>
        <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
      </w:sdtContent>
    </w:sdt>
    <w:p><w:r><w:t>After</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

#[test]
fn block_sdt_content_is_unwrapped_into_the_body() {
    let doc = parse_document(SDT_DOC).unwrap();
    let kinds: Vec<&str> = doc
        .body
        .children
        .iter()
        .map(|c| match c {
            DocxBodyChild::Paragraph(_) => "p",
            DocxBodyChild::Table(_) => "tbl",
        })
        .collect();
    // Before + (Inside one, Nested, table from the two controls) + After — every
    // content control's children are unwrapped in order, nothing dropped.
    assert_eq!(kinds, ["p", "p", "p", "tbl", "p"]);
}
