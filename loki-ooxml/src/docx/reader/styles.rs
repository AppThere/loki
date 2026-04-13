// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Reader for `word/styles.xml` → [`DocxStyles`].
//!
//! ECMA-376 §17.7 (document styles).

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::docx::model::styles::{DocxStyle, DocxStyleType, DocxStyles};
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

use super::document::parse_ppr_element;
use super::document::parse_rpr_element;

/// Parses `word/styles.xml` bytes into a [`DocxStyles`] model.
///
/// ECMA-376 §17.7.4.18.
pub fn parse_styles(xml: &[u8]) -> OoxmlResult<DocxStyles> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut result = DocxStyles::default();
    let mut buf = Vec::new();
    let mut in_style = false;
    let mut in_doc_defaults = false;
    let mut current_style: Option<DocxStyle> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"docDefaults" => in_doc_defaults = true,
                    b"rPrDefault" if in_doc_defaults => {}
                    b"pPrDefault" if in_doc_defaults => {}
                    b"rPr" if in_doc_defaults => {
                        if let Ok(rpr) = parse_rpr_element(&mut reader) {
                            result.default_rpr = Some(rpr);
                        }
                    }
                    b"pPr" if in_doc_defaults => {
                        if let Ok(ppr) = parse_ppr_element(&mut reader) {
                            result.default_ppr = Some(ppr);
                        }
                    }
                    b"style" => {
                        in_style = true;
                        let type_str = attr_val(e, b"type").unwrap_or_default();
                        let style_type = match type_str.as_str() {
                            "paragraph" => DocxStyleType::Paragraph,
                            "character" => DocxStyleType::Character,
                            "table" => DocxStyleType::Table,
                            "numbering" => DocxStyleType::Numbering,
                            _ => DocxStyleType::Paragraph,
                        };
                        let style_id =
                            attr_val(e, b"styleId").unwrap_or_default();
                        let is_default = attr_val(e, b"default")
                            .map_or(false, |v| v == "1" || v == "true");
                        let is_custom = attr_val(e, b"customStyle")
                            .map_or(false, |v| v == "1" || v == "true");
                        current_style = Some(DocxStyle {
                            style_type,
                            style_id,
                            is_default,
                            is_custom,
                            name: None,
                            based_on: None,
                            next: None,
                            link: None,
                            ppr: None,
                            rpr: None,
                        });
                    }
                    b"name" if in_style => {
                        if let Some(ref mut s) = current_style {
                            s.name = attr_val(e, b"val");
                        }
                    }
                    b"basedOn" if in_style => {
                        if let Some(ref mut s) = current_style {
                            s.based_on = attr_val(e, b"val");
                        }
                    }
                    b"next" if in_style => {
                        if let Some(ref mut s) = current_style {
                            s.next = attr_val(e, b"val");
                        }
                    }
                    b"link" if in_style => {
                        if let Some(ref mut s) = current_style {
                            s.link = attr_val(e, b"val");
                        }
                    }
                    b"pPr" if in_style => {
                        if let Ok(ppr) = parse_ppr_element(&mut reader) {
                            if let Some(ref mut s) = current_style {
                                s.ppr = Some(ppr);
                            }
                        }
                    }
                    b"rPr" if in_style => {
                        if let Ok(rpr) = parse_rpr_element(&mut reader) {
                            if let Some(ref mut s) = current_style {
                                s.rpr = Some(rpr);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"docDefaults" => in_doc_defaults = false,
                    b"style" => {
                        if let Some(style) = current_style.take() {
                            result.styles.push(style);
                        }
                        in_style = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "word/styles.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_STYLES: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Normal" w:default="1">
    <w:name w:val="Normal"/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="32"/></w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="DefaultParagraphFont" w:default="1">
    <w:name w:val="Default Paragraph Font"/>
  </w:style>
</w:styles>"#;

    #[test]
    fn parses_normal_style() {
        let styles = parse_styles(MINIMAL_STYLES).unwrap();
        assert!(styles.styles.iter().any(|s| s.style_id == "Normal" && s.is_default));
    }

    #[test]
    fn parses_heading1_based_on() {
        let styles = parse_styles(MINIMAL_STYLES).unwrap();
        let h1 = styles.styles.iter().find(|s| s.style_id == "Heading1").unwrap();
        assert_eq!(h1.based_on.as_deref(), Some("Normal"));
    }

    #[test]
    fn parses_character_style() {
        let styles = parse_styles(MINIMAL_STYLES).unwrap();
        assert!(styles
            .styles
            .iter()
            .any(|s| s.style_type == DocxStyleType::Character));
    }
}
