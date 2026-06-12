// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;
use crate::odt::model::styles::OdfParaProps;
use loki_doc_model::style::props::para_props::{LineHeight, ParagraphAlignment, Spacing};

#[test]
fn para_margins_to_spacing() {
    let props = OdfParaProps {
        margin_top: Some("6pt".into()),
        margin_bottom: Some("12pt".into()),
        margin_left: Some("1cm".into()),
        margin_right: Some("0.5cm".into()),
        ..Default::default()
    };
    let out = map_para_props(&props);
    assert!(
        matches!(out.space_before, Some(Spacing::Exact(p)) if (p.value() - 6.0).abs() < 1e-6)
    );
    assert!(
        matches!(out.space_after, Some(Spacing::Exact(p)) if (p.value() - 12.0).abs() < 1e-6)
    );
    assert!(out.indent_start.is_some());
    assert!(out.indent_end.is_some());
}

#[test]
fn text_indent_positive_is_first_line() {
    let props = OdfParaProps {
        text_indent: Some("0.5cm".into()),
        ..Default::default()
    };
    let out = map_para_props(&props);
    assert!(out.indent_first_line.is_some());
    assert!(out.indent_hanging.is_none());
}

#[test]
fn text_indent_negative_is_hanging() {
    let props = OdfParaProps {
        text_indent: Some("-0.5cm".into()),
        ..Default::default()
    };
    let out = map_para_props(&props);
    assert!(out.indent_hanging.is_some());
    assert!(out.indent_first_line.is_none());
    // hanging indent is stored as positive value
    let hanging = out.indent_hanging.unwrap().value();
    assert!(
        (hanging - crate::xml_util::parse_length("0.5cm").unwrap().value()).abs() < 1e-6,
        "expected 0.5cm ≈ {:.3}pt, got {:.3}pt",
        crate::xml_util::parse_length("0.5cm").unwrap().value(),
        hanging
    );
}

#[test]
fn line_height_percent() {
    let props = OdfParaProps {
        line_height: Some("150%".into()),
        ..Default::default()
    };
    let out = map_para_props(&props);
    assert!(
        matches!(out.line_height, Some(LineHeight::Multiple(m)) if (m - 1.5).abs() < 1e-5),
        "expected Multiple(1.5), got {:?}",
        out.line_height
    );
}

#[test]
fn line_height_exact_points() {
    let props = OdfParaProps {
        line_height: Some("14pt".into()),
        ..Default::default()
    };
    let out = map_para_props(&props);
    assert!(
        matches!(out.line_height, Some(LineHeight::Exact(p)) if (p.value() - 14.0).abs() < 1e-6),
        "expected Exact(14pt), got {:?}",
        out.line_height
    );
}

#[test]
fn line_height_at_least() {
    let props = OdfParaProps {
        line_height_at_least: Some("10pt".into()),
        ..Default::default()
    };
    let out = map_para_props(&props);
    assert!(
        matches!(out.line_height, Some(LineHeight::AtLeast(p)) if (p.value() - 10.0).abs() < 1e-6)
    );
}

#[test]
fn text_align_mappings() {
    let cases = [
        ("left", ParagraphAlignment::Left),
        ("start", ParagraphAlignment::Left),
        ("right", ParagraphAlignment::Right),
        ("end", ParagraphAlignment::Right),
        ("center", ParagraphAlignment::Center),
        ("justify", ParagraphAlignment::Justify),
        ("both", ParagraphAlignment::Justify),
    ];
    for (input, expected) in cases {
        let props = OdfParaProps {
            text_align: Some(input.into()),
            ..Default::default()
        };
        let out = map_para_props(&props);
        assert_eq!(out.alignment, Some(expected), "for input {:?}", input);
    }
}

#[test]
fn keep_together_and_keep_with_next() {
    let props = OdfParaProps {
        keep_together: Some("always".into()),
        keep_with_next: Some("always".into()),
        ..Default::default()
    };
    let out = map_para_props(&props);
    assert_eq!(out.keep_together, Some(true));
    assert_eq!(out.keep_with_next, Some(true));
}

#[test]
fn widows_orphans_break() {
    let props = OdfParaProps {
        widows: Some(2),
        orphans: Some(2),
        break_before: Some("page".into()),
        ..Default::default()
    };
    let out = map_para_props(&props);
    assert_eq!(out.widow_control, Some(2));
    assert_eq!(out.orphan_control, Some(2));
    assert_eq!(out.page_break_before, Some(true));
}

#[test]
fn parse_odf_border_solid_black() {
    let b = parse_odf_border("0.06pt solid #000000").expect("should parse");
    assert!(
        (b.width.value() - 0.06).abs() < 0.01,
        "width should be ~0.06pt, got {}",
        b.width.value()
    );
    use loki_doc_model::style::props::border::BorderStyle;
    assert_eq!(b.style, BorderStyle::Solid);
    assert!(b.color.is_some(), "color should be parsed");
}

#[test]
fn parse_odf_border_none_returns_none() {
    assert!(parse_odf_border("none").is_none());
}
