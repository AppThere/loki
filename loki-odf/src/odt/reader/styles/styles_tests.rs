// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Unit tests for `styles/mod.rs` — `read_stylesheet` and `read_auto_styles`.

use super::{read_auto_styles, read_stylesheet};
use crate::odt::model::list_styles::OdfListLevelKind;
use crate::odt::model::styles::OdfStyleFamily;

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
