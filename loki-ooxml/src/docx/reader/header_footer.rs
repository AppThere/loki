// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Reader for `word/header*.xml` and `word/footer*.xml` → `Vec<DocxParagraph>`.
//!
//! ECMA-376 §17.10.4 (`w:hdr`) / §17.10.2 (`w:ftr`).
//! Uses `quick-xml` event reader with `trim_text(false)` per ADR-0002.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::docx::model::paragraph::DocxParagraph;
use crate::docx::reader::document::parse_paragraph;
use crate::docx::reader::util::local_name;
use crate::error::{OoxmlError, OoxmlResult};

/// Parses a `word/header*.xml` or `word/footer*.xml` part.
///
/// Returns the ordered sequence of [`DocxParagraph`]s found inside the root
/// `<w:hdr>` or `<w:ftr>` element. The paragraph XML is identical in
/// structure to body paragraphs, so [`parse_paragraph`] is reused directly.
///
/// # Errors
///
/// Returns an error if the XML is malformed. Missing or empty header/footer
/// parts are treated as empty paragraph lists (not an error).
pub fn parse_header_footer(xml: &[u8], part: &str) -> OoxmlResult<Vec<DocxParagraph>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut paragraphs: Vec<DocxParagraph> = Vec::new();
    let mut in_root = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"hdr" | b"ftr" => in_root = true,
                b"p" if in_root => {
                    let para = parse_paragraph(&mut reader)?;
                    paragraphs.push(para);
                }
                _ => {}
            },
            Ok(Event::End(ref e)) => {
                if matches!(local_name(e.local_name().as_ref()), b"hdr" | b"ftr") {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: part.to_owned(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(paragraphs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_HEADER: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p>
    <w:r><w:t>Header text</w:t></w:r>
  </w:p>
</w:hdr>"#;

    #[test]
    fn parses_single_paragraph() {
        let paras = parse_header_footer(MINIMAL_HEADER, "word/header1.xml").unwrap();
        assert_eq!(paras.len(), 1, "expected one paragraph");
    }

    #[test]
    fn empty_hdr_produces_empty_vec() {
        let xml = br#"<?xml version="1.0"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#;
        let paras = parse_header_footer(xml, "word/header1.xml").unwrap();
        assert!(paras.is_empty());
    }

    const MINIMAL_FOOTER: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p><w:r><w:t>Footer</w:t></w:r></w:p>
  <w:p><w:r><w:t>Second paragraph</w:t></w:r></w:p>
</w:ftr>"#;

    #[test]
    fn footer_with_two_paragraphs() {
        let paras = parse_header_footer(MINIMAL_FOOTER, "word/footer1.xml").unwrap();
        assert_eq!(paras.len(), 2);
    }
}
