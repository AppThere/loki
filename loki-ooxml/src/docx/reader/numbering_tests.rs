// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the `word/numbering.xml` reader. Extracted from `numbering.rs`
//! (inline-test extraction to hold the 300-line ceiling).

use super::*;

const MINIMAL_NUMBERING: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0">
      <w:start w:val="1"/>
      <w:numFmt w:val="decimal"/>
      <w:lvlText w:val="%1."/>
      <w:lvlJc w:val="left"/>
    </w:lvl>
    <w:lvl w:ilvl="1">
      <w:start w:val="1"/>
      <w:numFmt w:val="bullet"/>
      <w:lvlText w:val="&#x2022;"/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1">
    <w:abstractNumId w:val="0"/>
  </w:num>
</w:numbering>"#;

#[test]
fn parses_abstract_num() {
    let numbering = parse_numbering(MINIMAL_NUMBERING).unwrap();
    assert_eq!(numbering.abstract_nums.len(), 1);
    assert_eq!(numbering.abstract_nums[0].levels.len(), 2);
}

#[test]
fn decimal_level_zero() {
    let numbering = parse_numbering(MINIMAL_NUMBERING).unwrap();
    let lvl = &numbering.abstract_nums[0].levels[0];
    assert_eq!(lvl.num_fmt.as_deref(), Some("decimal"));
    assert_eq!(lvl.lvl_text.as_deref(), Some("%1."));
}

#[test]
fn resolves_num_to_abstract() {
    let numbering = parse_numbering(MINIMAL_NUMBERING).unwrap();
    assert_eq!(numbering.abstract_num_id_for(1), Some(0));
}

#[test]
fn three_level_indirection() {
    let numbering = parse_numbering(MINIMAL_NUMBERING).unwrap();
    let resolved = numbering.resolve(1).unwrap();
    let lvl = resolved.level(0).unwrap();
    assert_eq!(lvl.num_fmt.as_deref(), Some("decimal"));
}

const PIC_BULLET_NUMBERING: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:numPicBullet w:numPicBulletId="0">
    <w:pict><v:shape><v:imagedata r:id="rId7"/></v:shape></w:pict>
  </w:numPicBullet>
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0">
      <w:numFmt w:val="bullet"/>
      <w:lvlText w:val="&#x2022;"/>
      <w:lvlPicBulletId w:val="0"/>
    </w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
</w:numbering>"#;

#[test]
fn parses_num_pic_bullet_and_lvl_reference() {
    let numbering = parse_numbering(PIC_BULLET_NUMBERING).unwrap();
    assert_eq!(numbering.pic_bullets.len(), 1);
    assert_eq!(numbering.pic_bullets[0].id, 0);
    assert_eq!(numbering.pic_bullets[0].rel_id, "rId7");
    assert_eq!(numbering.pic_bullets[0].src, None);
    let lvl = &numbering.abstract_nums[0].levels[0];
    assert_eq!(lvl.lvl_pic_bullet_id, Some(0));
}
