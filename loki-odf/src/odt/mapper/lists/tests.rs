// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::map_list_styles;
use crate::odt::model::list_styles::{OdfListLevel, OdfListLevelKind, OdfListStyle};
use crate::version::OdfVersion;
use crate::xml_util::parse_length;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{BulletChar, ListId, ListLevelKind, NumberingScheme};

fn bullet_level(ch: &str, legacy_space: &str, legacy_width: &str) -> OdfListLevel {
    OdfListLevel {
        level: 0,
        kind: OdfListLevelKind::Bullet {
            char: ch.into(),
            style_name: None,
        },
        legacy_space_before: Some(legacy_space.into()),
        legacy_min_label_width: Some(legacy_width.into()),
        legacy_min_label_distance: None,
        label_followed_by: None,
        list_tab_stop_position: None,
        text_indent: None,
        margin_left: None,
        text_props: None,
    }
}

fn number_level(
    fmt: &str,
    suffix: &str,
    start: u32,
    display_levels: u8,
    margin: &str,
    indent: &str,
) -> OdfListLevel {
    OdfListLevel {
        level: 0,
        kind: OdfListLevelKind::Number {
            num_format: Some(fmt.into()),
            num_prefix: None,
            num_suffix: Some(suffix.into()),
            start_value: Some(start),
            display_levels,
            style_name: None,
        },
        legacy_space_before: None,
        legacy_min_label_width: None,
        legacy_min_label_distance: None,
        label_followed_by: Some("listtab".into()),
        list_tab_stop_position: None,
        text_indent: Some(indent.into()),
        margin_left: Some(margin.into()),
        text_props: None,
    }
}

#[test]
fn bullet_char_bullet() {
    let level = bullet_level("•", "0.25cm", "0.25cm");
    let ls = OdfListStyle {
        name: "L1".into(),
        levels: vec![level],
    };
    let mut catalog = StyleCatalog::new();
    map_list_styles(&[ls], &mut catalog, OdfVersion::V1_1);
    let style = catalog.list_styles.get(&ListId::new("L1")).unwrap();
    assert_eq!(style.levels.len(), 1);
    match &style.levels[0].kind {
        ListLevelKind::Bullet {
            char: BulletChar::Char(c),
            ..
        } => {
            assert_eq!(*c, '•');
        }
        other => panic!("expected Bullet, got {:?}", other),
    }
}

#[test]
fn bullet_custom_char() {
    let level = bullet_level("-", "0.5cm", "0.25cm");
    let ls = OdfListStyle {
        name: "L2".into(),
        levels: vec![level],
    };
    let mut catalog = StyleCatalog::new();
    map_list_styles(&[ls], &mut catalog, OdfVersion::V1_1);
    let style = catalog.list_styles.get(&ListId::new("L2")).unwrap();
    match &style.levels[0].kind {
        ListLevelKind::Bullet {
            char: BulletChar::Char(c),
            ..
        } => {
            assert_eq!(*c, '-');
        }
        other => panic!("expected Bullet, got {:?}", other),
    }
}

#[test]
fn number_decimal_with_suffix() {
    let level = number_level("1", ".", 1, 1, "1.27cm", "-0.635cm");
    let ls = OdfListStyle {
        name: "L3".into(),
        levels: vec![level],
    };
    let mut catalog = StyleCatalog::new();
    map_list_styles(&[ls], &mut catalog, OdfVersion::V1_2);
    let style = catalog.list_styles.get(&ListId::new("L3")).unwrap();
    match &style.levels[0].kind {
        ListLevelKind::Numbered {
            scheme,
            format,
            start_value,
            ..
        } => {
            assert_eq!(*scheme, NumberingScheme::Decimal);
            assert_eq!(format, "%1.");
            assert_eq!(*start_value, 1);
        }
        other => panic!("expected Numbered, got {:?}", other),
    }
}

#[test]
fn number_lower_alpha() {
    let level = number_level("a", ")", 1, 1, "1.27cm", "-0.635cm");
    let ls = OdfListStyle {
        name: "L4".into(),
        levels: vec![level],
    };
    let mut catalog = StyleCatalog::new();
    map_list_styles(&[ls], &mut catalog, OdfVersion::V1_2);
    let style = catalog.list_styles.get(&ListId::new("L4")).unwrap();
    match &style.levels[0].kind {
        ListLevelKind::Numbered { scheme, format, .. } => {
            assert_eq!(*scheme, NumberingScheme::LowerAlpha);
            assert_eq!(format, "%1)");
        }
        other => panic!("expected Numbered, got {:?}", other),
    }
}

#[test]
fn odf12_label_alignment_indentation() {
    let level = number_level("1", ".", 1, 1, "1.27cm", "-0.635cm");
    let ls = OdfListStyle {
        name: "L5".into(),
        levels: vec![level],
    };
    let mut catalog = StyleCatalog::new();
    map_list_styles(&[ls], &mut catalog, OdfVersion::V1_2);
    let style = catalog.list_styles.get(&ListId::new("L5")).unwrap();
    let ll = &style.levels[0];
    // margin_left = 1.27cm ≈ 36.0pt
    assert!(
        ll.indent_start.value() > 35.0 && ll.indent_start.value() < 37.0,
        "indent_start={}",
        ll.indent_start.value()
    );
    // text_indent = -0.635cm ≈ 18pt, stored as positive hanging
    assert!(
        ll.hanging_indent.value() > 17.0 && ll.hanging_indent.value() < 19.0,
        "hanging_indent={}",
        ll.hanging_indent.value()
    );
}

#[test]
fn odf11_legacy_indentation() {
    // space_before=0.25cm, min_label_width=0.25cm
    // → indent_start = 0.5cm, hanging = 0.25cm
    let level = bullet_level("•", "0.25cm", "0.25cm");
    let ls = OdfListStyle {
        name: "L6".into(),
        levels: vec![level],
    };
    let mut catalog = StyleCatalog::new();
    map_list_styles(&[ls], &mut catalog, OdfVersion::V1_1);
    let style = catalog.list_styles.get(&ListId::new("L6")).unwrap();
    let ll = &style.levels[0];
    let expected_indent = parse_length("0.5cm").unwrap().value();
    let expected_hanging = parse_length("0.25cm").unwrap().value();
    assert!(
        (ll.indent_start.value() - expected_indent).abs() < 1e-4,
        "indent_start: expected {:.3}, got {:.3}",
        expected_indent,
        ll.indent_start.value()
    );
    assert!(
        (ll.hanging_indent.value() - expected_hanging).abs() < 1e-4,
        "hanging: expected {:.3}, got {:.3}",
        expected_hanging,
        ll.hanging_indent.value()
    );
}

#[test]
fn display_levels_two_format() {
    // level=1 (0-indexed), display_levels=2
    // → format "%1.%2."
    let level = OdfListLevel {
        level: 1, // 0-indexed → level_num=2
        kind: OdfListLevelKind::Number {
            num_format: Some("1".into()),
            num_prefix: None,
            num_suffix: Some(".".into()),
            start_value: Some(1),
            display_levels: 2,
            style_name: None,
        },
        legacy_space_before: None,
        legacy_min_label_width: None,
        legacy_min_label_distance: None,
        label_followed_by: Some("listtab".into()),
        list_tab_stop_position: None,
        text_indent: Some("-0.5cm".into()),
        margin_left: Some("1cm".into()),
        text_props: None,
    };
    let ls = OdfListStyle {
        name: "L7".into(),
        levels: vec![level],
    };
    let mut catalog = StyleCatalog::new();
    map_list_styles(&[ls], &mut catalog, OdfVersion::V1_2);
    let style = catalog.list_styles.get(&ListId::new("L7")).unwrap();
    match &style.levels[0].kind {
        ListLevelKind::Numbered {
            format,
            display_levels,
            ..
        } => {
            assert_eq!(format, "%1.%2.");
            assert_eq!(*display_levels, 2);
        }
        other => panic!("expected Numbered, got {:?}", other),
    }
}
