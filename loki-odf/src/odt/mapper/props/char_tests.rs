// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;
use crate::odt::model::styles::OdfTextProps;
use loki_doc_model::style::props::char_props::VerticalAlign;

#[test]
fn bold_true_false_none() {
    let bold = OdfTextProps {
        font_weight: Some("bold".into()),
        ..Default::default()
    };
    assert_eq!(map_text_props(&bold).bold, Some(true));

    let normal = OdfTextProps {
        font_weight: Some("normal".into()),
        ..Default::default()
    };
    assert_eq!(map_text_props(&normal).bold, Some(false));

    let absent = OdfTextProps::default();
    assert_eq!(map_text_props(&absent).bold, None);
}

#[test]
fn italic_mapping() {
    let italic = OdfTextProps {
        font_style: Some("italic".into()),
        ..Default::default()
    };
    assert_eq!(map_text_props(&italic).italic, Some(true));

    let normal = OdfTextProps {
        font_style: Some("normal".into()),
        ..Default::default()
    };
    assert_eq!(map_text_props(&normal).italic, Some(false));
}

#[test]
fn font_size_parsed() {
    let props = OdfTextProps {
        font_size: Some("12pt".into()),
        ..Default::default()
    };
    let out = map_text_props(&props);
    assert!(matches!(out.font_size, Some(p) if (p.value() - 12.0).abs() < 1e-6));
}

#[test]
fn underline_none_clears() {
    let props = OdfTextProps {
        text_underline_style: Some("none".into()),
        ..Default::default()
    };
    assert!(map_text_props(&props).underline.is_none());
}

#[test]
fn underline_solid_maps_to_single() {
    let props = OdfTextProps {
        text_underline_style: Some("solid".into()),
        ..Default::default()
    };
    assert_eq!(
        map_text_props(&props).underline,
        Some(UnderlineStyle::Single)
    );
}

#[test]
fn text_position_super_and_sub() {
    let sup = OdfTextProps {
        text_position: Some("super".into()),
        ..Default::default()
    };
    assert_eq!(
        map_text_props(&sup).vertical_align,
        Some(VerticalAlign::Superscript)
    );

    let sub = OdfTextProps {
        text_position: Some("sub".into()),
        ..Default::default()
    };
    assert_eq!(
        map_text_props(&sub).vertical_align,
        Some(VerticalAlign::Subscript)
    );
}

#[test]
fn text_position_positive_pct_is_super() {
    let props = OdfTextProps {
        text_position: Some("33%".into()),
        ..Default::default()
    };
    assert_eq!(
        map_text_props(&props).vertical_align,
        Some(VerticalAlign::Superscript)
    );
}

#[test]
fn text_position_negative_pct_is_sub() {
    let props = OdfTextProps {
        text_position: Some("-33%".into()),
        ..Default::default()
    };
    assert_eq!(
        map_text_props(&props).vertical_align,
        Some(VerticalAlign::Subscript)
    );
}

#[test]
fn small_caps_and_all_caps() {
    let props = OdfTextProps {
        font_variant: Some("small-caps".into()),
        text_transform: Some("uppercase".into()),
        ..Default::default()
    };
    let out = map_text_props(&props);
    assert_eq!(out.small_caps, Some(true));
    assert_eq!(out.all_caps, Some(true));
}

#[test]
fn language_with_country() {
    let props = OdfTextProps {
        language: Some("en".into()),
        country: Some("US".into()),
        ..Default::default()
    };
    let out = map_text_props(&props);
    assert_eq!(out.language.as_ref().map(|t| t.as_str()), Some("en-US"));
}

#[test]
fn language_without_country() {
    let props = OdfTextProps {
        language: Some("de".into()),
        ..Default::default()
    };
    let out = map_text_props(&props);
    assert_eq!(out.language.as_ref().map(|t| t.as_str()), Some("de"));
}

#[test]
fn color_hex_parsed() {
    let props = OdfTextProps {
        color: Some("#FF0000".into()),
        ..Default::default()
    };
    let out = map_text_props(&props);
    assert!(out.color.is_some());
}

#[test]
fn letter_spacing_parsed() {
    let props = OdfTextProps {
        letter_spacing: Some("0.5pt".into()),
        ..Default::default()
    };
    let out = map_text_props(&props);
    assert!(matches!(out.letter_spacing, Some(p) if (p.value() - 0.5).abs() < 1e-6));
}
