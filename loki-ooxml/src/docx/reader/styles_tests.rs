// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the `word/styles.xml` reader (extracted from
//! `styles.rs` to keep it under the file-size ceiling).

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

const TABLE_GRID_STYLE: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="table" w:styleId="TableGrid">
<w:name w:val="Table Grid"/>
<w:tblPr>
  <w:tblBorders>
    <w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/>
    <w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/>
    <w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/>
    <w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/>
    <w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/>
    <w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/>
  </w:tblBorders>
</w:tblPr>
  </w:style>
</w:styles>"#;

#[test]
fn parses_table_grid_borders() {
    let styles = parse_styles(TABLE_GRID_STYLE).unwrap();
    let b = styles
        .styles
        .iter()
        .find(|s| s.style_id == "TableGrid")
        .and_then(|s| s.table.as_ref())
        .and_then(|t| t.tbl_borders.as_ref())
        .expect("tblBorders parsed");
    // All six edges present, including the interior gridlines.
    assert!(b.top.is_some() && b.bottom.is_some() && b.left.is_some() && b.right.is_some());
    assert!(b.inside_h.is_some(), "insideH gridline parsed");
    assert!(b.inside_v.is_some(), "insideV gridline parsed");
    assert_eq!(b.inside_h.as_ref().unwrap().sz, Some(4));
}

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
