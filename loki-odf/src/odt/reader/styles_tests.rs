// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the ODT `styles.xml` reader (`super`). Extracted from
//! `styles.rs` (Phase 7.1 inline-test extraction; no production-code change).

use super::*;

#[test]
fn read_stylesheet_named_style_with_props() {
    let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <style:style style:name="Text_20_Body" style:family="paragraph"
                 style:display-name="Text Body">
      <style:paragraph-properties fo:margin-top="0.2cm" fo:margin-bottom="0.2cm"/>
      <style:text-properties fo:font-size="12pt" fo:font-weight="bold"/>
    </style:style>
  </office:styles>
</office:document-styles>"#;

    let sheet = read_stylesheet(xml, false).unwrap();
    assert_eq!(sheet.named_styles.len(), 1);
    let s = &sheet.named_styles[0];
    assert_eq!(s.name, "Text_20_Body");
    assert_eq!(s.display_name.as_deref(), Some("Text Body"));
    assert_eq!(s.family, OdfStyleFamily::Paragraph);
    assert!(!s.is_automatic);

    let pp = s.para_props.as_ref().unwrap();
    assert_eq!(pp.margin_top.as_deref(), Some("0.2cm"));
    assert_eq!(pp.margin_bottom.as_deref(), Some("0.2cm"));

    let tp = s.text_props.as_ref().unwrap();
    assert_eq!(tp.font_size.as_deref(), Some("12pt"));
    assert_eq!(tp.font_weight.as_deref(), Some("bold"));
}

#[test]
fn read_stylesheet_page_layout_num_format() {
    // `style:num-format` on `style:page-layout-properties` carries the
    // page-number numbering scheme.
    let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0">
  <office:automatic-styles>
    <style:page-layout style:name="Mpm1">
      <style:page-layout-properties fo:page-width="21cm" fo:page-height="29.7cm"
          style:num-format="i" style:print-orientation="portrait"/>
    </style:page-layout>
  </office:automatic-styles>
</office:document-styles>"#;

    let sheet = read_stylesheet(xml, false).unwrap();
    assert_eq!(sheet.page_layouts.len(), 1);
    let pl = &sheet.page_layouts[0];
    assert_eq!(pl.name, "Mpm1");
    assert_eq!(pl.num_format.as_deref(), Some("i"));
    assert_eq!(pl.page_width.as_deref(), Some("21cm"));
}

#[test]
fn read_stylesheet_drop_cap() {
    let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0">
  <office:styles>
    <style:style style:name="Drop" style:family="paragraph">
      <style:paragraph-properties>
        <style:drop-cap style:lines="3" style:length="2" style:distance="0.2cm"/>
      </style:paragraph-properties>
    </style:style>
  </office:styles>
</office:document-styles>"#;

    let sheet = read_stylesheet(xml, false).unwrap();
    let dc = sheet.named_styles[0]
        .para_props
        .as_ref()
        .unwrap()
        .drop_cap
        .as_ref()
        .expect("drop cap parsed");
    assert_eq!(dc.lines.as_deref(), Some("3"));
    assert_eq!(dc.length.as_deref(), Some("2"));
    assert_eq!(dc.distance.as_deref(), Some("0.2cm"));
}

#[test]
fn read_stylesheet_graphic_wrap() {
    let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0">
  <office:automatic-styles>
    <style:style style:name="fr1" style:family="graphic">
      <style:graphic-properties style:wrap="left" style:run-through="foreground"/>
    </style:style>
  </office:automatic-styles>
</office:document-styles>"#;

    let sheet = read_stylesheet(xml, false).unwrap();
    let gw = sheet.auto_styles[0]
        .graphic_wrap
        .as_ref()
        .expect("graphic wrap parsed");
    assert_eq!(gw.wrap.as_deref(), Some("left"));
    assert_eq!(gw.run_through.as_deref(), Some("foreground"));
}

#[test]
fn read_stylesheet_list_style_bullet_level() {
    let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <text:list-style style:name="List_Bullet">
      <text:list-level-style-bullet text:level="1" text:bullet-char="-">
        <style:list-level-properties
            text:list-level-position-and-space-mode="label-alignment">
          <style:list-level-label-alignment
              text:label-followed-by="listtab"
              text:list-tab-stop-position="0.635cm"
              fo:text-indent="-0.635cm"
              fo:margin-left="0.635cm"/>
        </style:list-level-properties>
      </text:list-level-style-bullet>
    </text:list-style>
  </office:styles>
</office:document-styles>"#;

    let sheet = read_stylesheet(xml, false).unwrap();
    assert_eq!(sheet.list_styles.len(), 1);
    let ls = &sheet.list_styles[0];
    assert_eq!(ls.name, "List_Bullet");
    assert_eq!(ls.levels.len(), 1);

    let lv = &ls.levels[0];
    assert_eq!(lv.level, 0);
    assert!(matches!(lv.kind, OdfListLevelKind::Bullet { ref char, .. } if char == "-"));
    assert_eq!(lv.label_followed_by.as_deref(), Some("listtab"));
    assert_eq!(lv.list_tab_stop_position.as_deref(), Some("0.635cm"));
    assert_eq!(lv.text_indent.as_deref(), Some("-0.635cm"));
    assert_eq!(lv.margin_left.as_deref(), Some("0.635cm"));
}

#[test]
fn read_stylesheet_list_style_image_level() {
    // A picture-bullet level (feature 5.4), self-closing form — the reader
    // captures its `xlink:href` into `OdfListLevelKind::Image`.
    let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:xlink="http://www.w3.org/1999/xlink"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <text:list-style style:name="List_Pic">
      <text:list-level-style-image text:level="1" xlink:href="Pictures/bul.png"/>
    </text:list-style>
  </office:styles>
</office:document-styles>"#;

    let sheet = read_stylesheet(xml, false).unwrap();
    let lv = &sheet.list_styles[0].levels[0];
    assert_eq!(lv.level, 0);
    assert!(
        matches!(lv.kind, OdfListLevelKind::Image { ref href, .. } if href == "Pictures/bul.png"),
        "expected image level, got {:?}",
        lv.kind
    );
}

#[test]
fn read_stylesheet_list_style_number_label_alignment() {
    let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <text:list-style style:name="Numbered_List">
      <text:list-level-style-number text:level="1"
          style:num-format="1" style:num-suffix="." text:start-value="1">
        <style:list-level-properties
            text:list-level-position-and-space-mode="label-alignment">
          <style:list-level-label-alignment
              text:label-followed-by="listtab"
              text:list-tab-stop-position="1.27cm"
              fo:text-indent="-0.635cm"
              fo:margin-left="1.27cm"/>
        </style:list-level-properties>
      </text:list-level-style-number>
    </text:list-style>
  </office:styles>
</office:document-styles>"#;

    let sheet = read_stylesheet(xml, false).unwrap();
    let ls = &sheet.list_styles[0];
    let lv = &ls.levels[0];
    assert!(
        matches!(lv.kind, OdfListLevelKind::Number { ref num_format, .. }
                if num_format.as_deref() == Some("1"))
    );
    assert_eq!(lv.label_followed_by.as_deref(), Some("listtab"));
    assert_eq!(lv.margin_left.as_deref(), Some("1.27cm"));
}

#[test]
fn read_auto_styles_returns_automatic_styles() {
    let xml = br#"<?xml version="1.0"?>
<office:document-content
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
    office:version="1.3">
  <office:automatic-styles>
    <style:style style:name="P1" style:family="paragraph"
                 style:parent-style-name="Default_20_Paragraph_20_Style">
      <style:paragraph-properties fo:margin-left="0.5cm"/>
      <style:text-properties fo:font-size="10pt"/>
    </style:style>
    <style:style style:name="T1" style:family="text"/>
  </office:automatic-styles>
  <office:body>
    <office:text/>
  </office:body>
</office:document-content>"#;

    let styles = read_auto_styles(xml).unwrap();
    assert_eq!(styles.len(), 2);

    let p1 = &styles[0];
    assert_eq!(p1.name, "P1");
    assert_eq!(p1.family, OdfStyleFamily::Paragraph);
    assert!(p1.is_automatic);
    assert_eq!(
        p1.parent_name.as_deref(),
        Some("Default_20_Paragraph_20_Style")
    );
    let pp = p1.para_props.as_ref().unwrap();
    assert_eq!(pp.margin_left.as_deref(), Some("0.5cm"));
    let tp = p1.text_props.as_ref().unwrap();
    assert_eq!(tp.font_size.as_deref(), Some("10pt"));

    let t1 = &styles[1];
    assert_eq!(t1.name, "T1");
    assert_eq!(t1.family, OdfStyleFamily::Text);
    assert!(t1.is_automatic);
    assert!(t1.para_props.is_none());
}

#[test]
fn read_stylesheet_list_style_legacy_positioning() {
    let xml = br#"<?xml version="1.0"?>
<office:document-styles
    xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
    xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
    xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:styles>
    <text:list-style style:name="ODF11_List">
      <text:list-level-style-bullet text:level="1" text:bullet-char="-">
        <style:list-level-properties
            text:space-before="0.25cm"
            text:min-label-width="0.4cm"
            text:min-label-distance="0.1cm"/>
      </text:list-level-style-bullet>
    </text:list-style>
  </office:styles>
</office:document-styles>"#;

    let sheet = read_stylesheet(xml, false).unwrap();
    let lv = &sheet.list_styles[0].levels[0];
    assert_eq!(lv.legacy_space_before.as_deref(), Some("0.25cm"));
    assert_eq!(lv.legacy_min_label_width.as_deref(), Some("0.4cm"));
    assert_eq!(lv.legacy_min_label_distance.as_deref(), Some("0.1cm"));
    assert!(lv.label_followed_by.is_none());
}
