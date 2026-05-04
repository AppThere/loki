// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Paragraph and character property mappers.
//!
//! Converts [`OdfParaProps`] → [`ParaProps`] and
//! [`OdfTextProps`] → [`CharProps`].
//! All ODF measurement values are length strings (e.g. `"2.5cm"`, `"12pt"`);
//! conversion uses [`crate::xml_util::parse_length`].

use loki_doc_model::meta::LanguageTag;
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle, UnderlineStyle, VerticalAlign,
};
use loki_doc_model::style::props::para_props::{
    LineHeight, ParagraphAlignment, ParaProps, Spacing,
};
use loki_primitives::color::DocumentColor;

use crate::odt::model::styles::{OdfParaProps, OdfTextProps};
use crate::xml_util::parse_length;

// ── Paragraph properties ───────────────────────────────────────────────────────

/// Convert [`OdfParaProps`] to the format-neutral [`ParaProps`].
///
/// All length values (margins, text-indent, line-height) are parsed from ODF
/// attribute strings via [`parse_length`]. Unmapped or unparseable values are
/// silently dropped (the corresponding field remains `None`). ODF 1.3 §17.6.
pub(crate) fn map_para_props(props: &OdfParaProps) -> ParaProps {
    let mut out = ParaProps::default();

    // ── Spacing ────────────────────────────────────────────────────────────
    if let Some(s) = props.margin_top.as_deref().and_then(parse_length) {
        out.space_before = Some(Spacing::Exact(s));
    }
    if let Some(s) = props.margin_bottom.as_deref().and_then(parse_length) {
        out.space_after = Some(Spacing::Exact(s));
    }

    // ── Indentation ────────────────────────────────────────────────────────
    if let Some(pts) = props.margin_left.as_deref().and_then(parse_length) {
        out.indent_start = Some(pts);
    }
    if let Some(pts) = props.margin_right.as_deref().and_then(parse_length) {
        out.indent_end = Some(pts);
    }
    if let Some(raw) = props.text_indent.as_deref()
        && let Some(pts) = parse_length(raw)
    {
        let v = pts.value();
        if v < 0.0 {
            // Negative text-indent = hanging indent (stored as positive)
            out.indent_hanging =
                Some(loki_primitives::units::Points::new(-v));
        } else {
            out.indent_first_line = Some(pts);
        }
    }

    // ── Line height ────────────────────────────────────────────────────────
    if let Some(raw) = props.line_height.as_deref() {
        if let Some(pct_str) = raw.strip_suffix('%') {
            if let Ok(pct) = pct_str.trim().parse::<f32>() {
                out.line_height = Some(LineHeight::Multiple(pct / 100.0));
            }
        } else if let Some(pts) = parse_length(raw) {
            out.line_height = Some(LineHeight::Exact(pts));
        }
    }
    if let Some(pts) =
        props.line_height_at_least.as_deref().and_then(parse_length)
    {
        // Only set if line_height wasn't already set from fo:line-height
        if out.line_height.is_none() {
            out.line_height = Some(LineHeight::AtLeast(pts));
        }
    }

    // ── Alignment ──────────────────────────────────────────────────────────
    if let Some(align) = props.text_align.as_deref().map(map_text_align) {
        out.alignment = Some(align);
    }

    // ── Flow control ───────────────────────────────────────────────────────
    if props.keep_together.as_deref() == Some("always") {
        out.keep_together = Some(true);
    }
    if props.keep_with_next.as_deref() == Some("always") {
        out.keep_with_next = Some(true);
    }

    // ── Widow / orphan control ─────────────────────────────────────────────
    out.widow_control = props.widows;
    out.orphan_control = props.orphans;

    // ── Page breaks ────────────────────────────────────────────────────────
    if props.break_before.as_deref() == Some("page") {
        out.page_break_before = Some(true);
    }
    if props.break_after.as_deref() == Some("page") {
        out.page_break_after = Some(true);
    }

    out
}

/// Map an ODF `fo:text-align` string to [`ParagraphAlignment`].
fn map_text_align(s: &str) -> ParagraphAlignment {
    match s {
        "right" | "end" => ParagraphAlignment::Right,
        "center" => ParagraphAlignment::Center,
        "justify" | "both" => ParagraphAlignment::Justify,
        _ => ParagraphAlignment::Left,
    }
}

// ── Character properties ──────────────────────────────────────────────────────

/// Convert [`OdfTextProps`] to the format-neutral [`CharProps`].
///
/// ODF 1.3 §20.2 (`style:text-properties`).
pub(crate) fn map_text_props(props: &OdfTextProps) -> CharProps {
    // ── Font ───────────────────────────────────────────────────────────────
    // Prefer style:font-name (the font face alias, typically matching the actual
    // family name); fall back to fo:font-family when only that is present.
    let mut out = CharProps {
        font_name: props.font_name.clone().or_else(|| props.font_family.clone()),
        font_name_complex: props.font_name_complex.clone(),
        ..Default::default()
    };

    if let Some(pts) = props.font_size.as_deref().and_then(parse_length) {
        out.font_size = Some(pts);
    }
    if let Some(pts) =
        props.font_size_complex.as_deref().and_then(parse_length)
    {
        out.font_size_complex = Some(pts);
    }

    // ── Style flags ────────────────────────────────────────────────────────
    out.bold = match props.font_weight.as_deref() {
        Some("bold") => Some(true),
        Some("normal") => Some(false),
        _ => None,
    };
    out.italic = match props.font_style.as_deref() {
        Some("italic" | "oblique") => Some(true),
        Some("normal") => Some(false),
        _ => None,
    };
    out.underline = props
        .text_underline_style
        .as_deref()
        .and_then(map_underline_style);
    out.strikethrough = props
        .text_line_through_style
        .as_deref()
        .and_then(map_strikethrough_style);

    // ── Case / variant ─────────────────────────────────────────────────────
    if props.font_variant.as_deref() == Some("small-caps") {
        out.small_caps = Some(true);
    }
    if props.text_transform.as_deref() == Some("uppercase") {
        out.all_caps = Some(true);
    }

    // ── Vertical alignment (super/subscript) ───────────────────────────────
    if let Some(pos) = props.text_position.as_deref() {
        out.vertical_align = map_text_position(pos);
    }

    // ── Color ──────────────────────────────────────────────────────────────
    if let Some(hex) = props.color.as_deref()
        && let Ok(dc) = DocumentColor::from_hex(hex)
    {
        out.color = Some(dc);
    }
    if let Some(hex) = props.background_color.as_deref()
        && hex != "transparent"
        && let Ok(dc) = DocumentColor::from_hex(hex)
    {
        out.background_color = Some(dc);
    }

    // ── Spacing ────────────────────────────────────────────────────────────
    if let Some(pts) = props.letter_spacing.as_deref().and_then(parse_length)
    {
        out.letter_spacing = Some(pts);
    }

    // ── Language ───────────────────────────────────────────────────────────
    if let Some(lang) = props.language.as_deref() {
        let tag = if let Some(country) = props.country.as_deref() {
            LanguageTag::new(format!("{lang}-{country}"))
        } else {
            LanguageTag::new(lang)
        };
        out.language = Some(tag);
    }

    out
}

/// Map ODF `style:text-underline-style` to [`UnderlineStyle`].
///
/// `"none"` → `None` (explicit removal). All other recognised values map to
/// a concrete style; unrecognised values map to [`UnderlineStyle::Single`].
fn map_underline_style(s: &str) -> Option<UnderlineStyle> {
    match s {
        "none" => None,
        "double" => Some(UnderlineStyle::Double),
        "dotted" => Some(UnderlineStyle::Dotted),
        "dash" | "long-dash" | "dot-dash" | "dot-dot-dash" => {
            Some(UnderlineStyle::Dash)
        }
        "wave" => Some(UnderlineStyle::Wave),
        "bold" => Some(UnderlineStyle::Thick),
        _ => Some(UnderlineStyle::Single),
    }
}

/// Map ODF `style:text-line-through-style` to [`StrikethroughStyle`].
///
/// `"none"` → `None`. `"double"` → `Double`. All other values → `Single`.
fn map_strikethrough_style(s: &str) -> Option<StrikethroughStyle> {
    match s {
        "none" => None,
        "double" => Some(StrikethroughStyle::Double),
        _ => Some(StrikethroughStyle::Single),
    }
}

/// Map ODF `style:text-position` to [`VerticalAlign`].
///
/// Recognised forms: `"super"`, `"sub"`, percentage strings (positive =
/// superscript, negative = subscript), or a percentage followed by a font
/// size (the second token is ignored). ODF 1.3 §19.879.
fn map_text_position(s: &str) -> Option<VerticalAlign> {
    let first = s.split_whitespace().next().unwrap_or(s);
    match first {
        "super" => Some(VerticalAlign::Superscript),
        "sub" => Some(VerticalAlign::Subscript),
        "0%" | "0" => Some(VerticalAlign::Baseline),
        other => {
            // Percentage string: positive → super, negative → sub
            if let Some(pct_str) = other.strip_suffix('%')
                && let Ok(pct) = pct_str.parse::<f32>()
            {
                return if pct > 0.0 {
                    Some(VerticalAlign::Superscript)
                } else if pct < 0.0 {
                    Some(VerticalAlign::Subscript)
                } else {
                    Some(VerticalAlign::Baseline)
                };
            }
            None
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::odt::model::styles::{OdfParaProps, OdfTextProps};
    use loki_doc_model::style::props::char_props::VerticalAlign;
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
            assert_eq!(
                out.alignment,
                Some(expected),
                "for input {:?}",
                input
            );
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
        let bold = OdfTextProps { font_weight: Some("bold".into()), ..Default::default() };
        assert_eq!(map_text_props(&bold).bold, Some(true));

        let normal = OdfTextProps { font_weight: Some("normal".into()), ..Default::default() };
        assert_eq!(map_text_props(&normal).bold, Some(false));

        let absent = OdfTextProps::default();
        assert_eq!(map_text_props(&absent).bold, None);
    }

    #[test]
    fn italic_mapping() {
        let italic = OdfTextProps { font_style: Some("italic".into()), ..Default::default() };
        assert_eq!(map_text_props(&italic).italic, Some(true));

        let normal = OdfTextProps { font_style: Some("normal".into()), ..Default::default() };
        assert_eq!(map_text_props(&normal).italic, Some(false));
    }

    #[test]
    fn font_size_parsed() {
        let props = OdfTextProps { font_size: Some("12pt".into()), ..Default::default() };
        let out = map_text_props(&props);
        assert!(
            matches!(out.font_size, Some(p) if (p.value() - 12.0).abs() < 1e-6)
        );
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
        assert!(
            matches!(out.letter_spacing, Some(p) if (p.value() - 0.5).abs() < 1e-6)
        );
    }
}
