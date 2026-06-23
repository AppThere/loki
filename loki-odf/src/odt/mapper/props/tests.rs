// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the property mappers.

use super::cell::{map_odf_vertical_align, map_odf_writing_mode};
use super::paragraph::parse_odf_border;
use super::*;
use crate::odt::model::styles::{OdfParaProps, OdfTextProps};
use loki_doc_model::content::table::row::{CellTextDirection, CellVerticalAlign};
use loki_doc_model::style::props::char_props::{UnderlineStyle, VerticalAlign};
use loki_doc_model::style::props::para_props::{LineHeight, ParagraphAlignment, Spacing};

// ── map_para_props ─────────────────────────────────────────────────────

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
    assert!(matches!(out.space_before, Some(Spacing::Exact(p)) if (p.value() - 6.0).abs() < 1e-6));
    assert!(matches!(out.space_after, Some(Spacing::Exact(p)) if (p.value() - 12.0).abs() < 1e-6));
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

// ── map_text_props ─────────────────────────────────────────────────────

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

// ── cell property helpers ──────────────────────────────────────────────

#[test]
fn vertical_align_middle_maps_to_middle() {
    assert_eq!(
        map_odf_vertical_align("middle"),
        Some(CellVerticalAlign::Middle)
    );
}

#[test]
fn vertical_align_top_maps_to_top() {
    assert_eq!(map_odf_vertical_align("top"), Some(CellVerticalAlign::Top));
}

#[test]
fn vertical_align_automatic_maps_to_top() {
    assert_eq!(
        map_odf_vertical_align("automatic"),
        Some(CellVerticalAlign::Top)
    );
}

#[test]
fn vertical_align_bottom_maps_to_bottom() {
    assert_eq!(
        map_odf_vertical_align("bottom"),
        Some(CellVerticalAlign::Bottom)
    );
}

#[test]
fn vertical_align_unknown_returns_none() {
    assert_eq!(map_odf_vertical_align("baseline"), None);
}

#[test]
fn writing_mode_tb_rl_maps_to_tbrl() {
    assert_eq!(map_odf_writing_mode("tb-rl"), Some(CellTextDirection::TbRl));
}

#[test]
fn writing_mode_lr_tb_maps_to_lrtb() {
    assert_eq!(map_odf_writing_mode("lr-tb"), Some(CellTextDirection::LrTb));
}

#[test]
fn writing_mode_lr_shorthand_maps_to_lrtb() {
    assert_eq!(map_odf_writing_mode("lr"), Some(CellTextDirection::LrTb));
}

#[test]
fn parse_odf_border_solid_black() {
    let b = parse_odf_border("0.06pt solid #000000").expect("should parse");
    // Width rounds to 0.06pt
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

#[test]
fn fo_padding_shorthand_applies_to_all_edges() {
    use crate::odt::model::styles::OdfCellProps;

    let cell_props = OdfCellProps {
        padding_top: Some("0.2cm".into()),
        padding_bottom: Some("0.2cm".into()),
        padding_left: Some("0.2cm".into()),
        padding_right: Some("0.2cm".into()),
        ..Default::default()
    };
    let props = map_cell_props(&cell_props);
    // 0.2cm ≈ 5.669pt
    for (label, val) in [
        ("top", props.padding_top),
        ("bottom", props.padding_bottom),
        ("left", props.padding_left),
        ("right", props.padding_right),
    ] {
        let pts = val
            .expect(&format!("padding_{label} should be Some"))
            .value();
        assert!(
            (pts - 5.669).abs() < 0.1,
            "padding_{label} should be ~5.67pt, got {pts:.3}"
        );
    }
}
