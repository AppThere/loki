// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Reader for `word/document.xml` → [`DocxDocument`].
//!
//! ECMA-376 §17.2 (document structure), §17.3 (block-level content).
//! Uses `quick-xml` event reader with `trim_text(false)` per ADR-0002.

mod para;
mod run;
mod sect;
mod table;

pub(crate) use para::{parse_paragraph, parse_ppr_element};
pub(crate) use run::parse_rpr_element;

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::model::document::{DocxBodyChild, DocxDocument};
use crate::docx::reader::util::local_name;
use crate::error::{OoxmlError, OoxmlResult};

/// Parses `word/document.xml` bytes into a [`DocxDocument`].
pub fn parse_document(xml: &[u8]) -> OoxmlResult<DocxDocument> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut doc = DocxDocument::default();
    let mut in_body = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"body" => in_body = true,
                b"p" if in_body => {
                    let para = para::parse_paragraph(&mut reader)?;
                    doc.body.children.push(DocxBodyChild::Paragraph(para));
                }
                b"tbl" if in_body => {
                    let tbl = table::parse_table(&mut reader)?;
                    doc.body.children.push(DocxBodyChild::Table(tbl));
                }
                b"sdt" if in_body => {
                    sect::skip_element(&mut reader, b"sdt")?;
                    doc.body.children.push(DocxBodyChild::Sdt);
                }
                b"sectPr" if in_body => {
                    let sect = sect::parse_sect_pr(&mut reader)?;
                    doc.body.final_sect_pr = Some(sect);
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => {
                if local_name(e.local_name().as_ref()) == b"body" {
                    in_body = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/document.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::model::document::DocxBodyChild;

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
}
