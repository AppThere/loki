// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the `word/document.xml` reader, extracted from `document.rs` to
//! hold the 300-line ceiling.

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
