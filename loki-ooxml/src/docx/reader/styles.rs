// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `word/styles.xml` → [`DocxStyles`]. ECMA-376 §17.7.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::model::styles::{
    DocxStyle, DocxStyleType, DocxStyles, DocxTableStyleProps, DocxTblStylePr,
};
use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};

use super::document::parse_ppr_element;
use super::document::parse_rpr_element;

/// Parses `word/styles.xml` bytes into a [`DocxStyles`] model (§17.7.4.18).
// Function body is a single large match over XML events; splitting would reduce readability.
#[allow(clippy::too_many_lines)]
pub fn parse_styles(xml: &[u8]) -> OoxmlResult<DocxStyles> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);

    let mut result = DocxStyles::default();
    let mut buf = Vec::new();
    let mut in_style = false;
    let mut in_doc_defaults = false;
    let mut current_style: Option<DocxStyle> = None;
    // Table-style parsing state: the `w:tblStylePr` region currently open (its
    // `@w:type`), and whether we are inside a `w:tcPr` (to scope `w:shd`).
    let mut current_region: Option<String> = None;
    let mut in_tcpr = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                match local_name(e.local_name().as_ref()) {
                    b"docDefaults" => in_doc_defaults = true,
                    b"rPrDefault" | b"pPrDefault" if in_doc_defaults => {}
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
                            "character" => DocxStyleType::Character,
                            "table" => DocxStyleType::Table,
                            "numbering" => DocxStyleType::Numbering,
                            _ => DocxStyleType::Paragraph,
                        };
                        let style_id = attr_val(e, b"styleId").unwrap_or_default();
                        let is_default =
                            attr_val(e, b"default").is_some_and(|v| v == "1" || v == "true");
                        let is_custom =
                            attr_val(e, b"customStyle").is_some_and(|v| v == "1" || v == "true");
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
                            table: (style_type == DocxStyleType::Table)
                                .then(DocxTableStyleProps::default),
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
                        if let Ok(ppr) = parse_ppr_element(&mut reader)
                            && let Some(ref mut s) = current_style
                        {
                            s.ppr = Some(ppr);
                        }
                    }
                    b"rPr" if in_style => {
                        if let Ok(rpr) = parse_rpr_element(&mut reader)
                            && let Some(ref mut s) = current_style
                        {
                            // Inside w:tblStylePr the rPr belongs to the
                            // REGION (4a.3; it used to clobber style rpr).
                            if current_region.is_some() {
                                if let Some(t) = s.table.as_mut()
                                    && let Some(last) = t.conditional.last_mut()
                                {
                                    last.rpr = Some(rpr);
                                }
                            } else {
                                s.rpr = Some(rpr);
                            }
                        }
                    }
                    b"tblStyleRowBandSize" if in_style => {
                        if let Some(t) = table_props_mut(&mut current_style) {
                            t.row_band_size = attr_val(e, b"val").and_then(|v| v.parse().ok());
                        }
                    }
                    b"tblStyleColBandSize" if in_style => {
                        if let Some(t) = table_props_mut(&mut current_style) {
                            t.col_band_size = attr_val(e, b"val").and_then(|v| v.parse().ok());
                        }
                    }
                    b"tblStylePr" if in_style => {
                        let region = attr_val(e, b"type").unwrap_or_default();
                        current_region = Some(region.clone());
                        if let Some(t) = table_props_mut(&mut current_style) {
                            t.conditional.push(DocxTblStylePr {
                                region,
                                shd_fill: None,
                                rpr: None,
                            });
                        }
                    }
                    b"tcPr" if in_style => in_tcpr = true,
                    b"shd" if in_style && in_tcpr => {
                        let fill = attr_val(e, b"fill");
                        if let Some(t) = table_props_mut(&mut current_style) {
                            if current_region.is_some() {
                                if let Some(last) = t.conditional.last_mut() {
                                    last.shd_fill = fill;
                                }
                            } else {
                                t.base_shd_fill = fill;
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => match local_name(e.local_name().as_ref()) {
                b"docDefaults" => in_doc_defaults = false,
                b"tcPr" => in_tcpr = false,
                b"tblStylePr" => current_region = None,
                b"style" => {
                    if let Some(style) = current_style.take() {
                        result.styles.push(style);
                    }
                    in_style = false;
                    current_region = None;
                    in_tcpr = false;
                }
                _ => {}
            },
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

/// Mutable access to the in-progress style's table-props accumulator, if the
/// current style is a table style.
fn table_props_mut(style: &mut Option<DocxStyle>) -> Option<&mut DocxTableStyleProps> {
    style.as_mut().and_then(|s| s.table.as_mut())
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
        assert!(
            styles
                .styles
                .iter()
                .any(|s| s.style_id == "Normal" && s.is_default)
        );
    }

    #[test]
    fn parses_heading1_based_on() {
        let styles = parse_styles(MINIMAL_STYLES).unwrap();
        let h1 = styles
            .styles
            .iter()
            .find(|s| s.style_id == "Heading1")
            .unwrap();
        assert_eq!(h1.based_on.as_deref(), Some("Normal"));
    }

    #[test]
    fn parses_character_style() {
        let styles = parse_styles(MINIMAL_STYLES).unwrap();
        assert!(
            styles
                .styles
                .iter()
                .any(|s| s.style_type == DocxStyleType::Character)
        );
    }

    const TABLE_STYLE: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="table" w:styleId="Banded">
    <w:name w:val="Banded"/>
    <w:tblPr>
      <w:tblStyleRowBandSize w:val="2"/>
      <w:tblStyleColBandSize w:val="1"/>
    </w:tblPr>
    <w:tcPr><w:shd w:val="clear" w:fill="FFFFFF"/></w:tcPr>
    <w:tblStylePr w:type="firstRow">
      <w:rPr><w:b/></w:rPr>
      <w:tcPr><w:shd w:val="clear" w:fill="4472C4"/></w:tcPr>
    </w:tblStylePr>
    <w:tblStylePr w:type="band1Horz">
      <w:tcPr><w:shd w:val="clear" w:fill="D9E2F3"/></w:tcPr>
    </w:tblStylePr>
  </w:style>
</w:styles>"#;

    #[test]
    fn parses_table_style_banding() {
        let styles = parse_styles(TABLE_STYLE).unwrap();
        let t = styles
            .styles
            .iter()
            .find(|s| s.style_id == "Banded")
            .and_then(|s| s.table.as_ref())
            .expect("table props parsed");
        assert_eq!(t.row_band_size, Some(2));
        assert_eq!(t.col_band_size, Some(1));
        assert_eq!(t.base_shd_fill.as_deref(), Some("FFFFFF"));
        assert_eq!(t.conditional.len(), 2);
        let first_row = t
            .conditional
            .iter()
            .find(|c| c.region == "firstRow")
            .unwrap();
        assert_eq!(first_row.shd_fill.as_deref(), Some("4472C4"));
        let band = t
            .conditional
            .iter()
            .find(|c| c.region == "band1Horz")
            .unwrap();
        assert_eq!(band.shd_fill.as_deref(), Some("D9E2F3"));
    }

    #[test]
    fn non_table_style_has_no_table_props() {
        let styles = parse_styles(MINIMAL_STYLES).unwrap();
        assert!(styles.styles.iter().all(|s| s.table.is_none()));
    }
}
