// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Paragraph and character property mappers.
//!
//! Converts [`DocxPPr`] → [`ParaProps`] and [`DocxRPr`] → [`CharProps`].
//! All OOXML measurements are in twentieths of a point (twips); all model
//! measurements are in [`loki_primitives::units::Points`].

use loki_doc_model::meta::LanguageTag;
use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::char_props::{
    CharProps, HighlightColor, StrikethroughStyle, UnderlineStyle, VerticalAlign,
};
use loki_doc_model::style::props::para_props::{LineHeight, ParagraphAlignment, ParaProps, Spacing};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use crate::docx::model::paragraph::{DocxPPr, DocxRPr};
use crate::xml_util::hex_color;

// ── Internal conversion helpers ───────────────────────────────────────────────

/// Converts a twips integer to [`Points`] (1 pt = 20 twips).
fn twips_to_pt(twips: i32) -> Points {
    Points::new(twips as f64 / 20.0)
}

/// Maps a `w:jc` value string to [`ParagraphAlignment`].
fn map_jc(jc: &str) -> ParagraphAlignment {
    match jc {
        "both" | "distribute" => ParagraphAlignment::Justify,
        "center" => ParagraphAlignment::Center,
        "right" | "end" => ParagraphAlignment::Right,
        _ => ParagraphAlignment::Left,
    }
}

/// Maps `w:line` + `w:lineRule` to [`LineHeight`].
///
/// - `lineRule="exact"` → [`LineHeight::Exact`] (pt)
/// - `lineRule="atLeast"` → [`LineHeight::AtLeast`] (pt)
/// - `lineRule="auto"` or absent → [`LineHeight::Multiple`] (line/240.0)
fn map_line_height(line: i32, line_rule: Option<&str>) -> LineHeight {
    match line_rule {
        Some("exact") => LineHeight::Exact(twips_to_pt(line)),
        Some("atLeast") => LineHeight::AtLeast(twips_to_pt(line)),
        _ => LineHeight::Multiple(line as f32 / 240.0),
    }
}

/// Maps a `w:u @w:val` string to [`UnderlineStyle`].
///
/// Returns `None` for `"none"` (explicit removal of underline).
fn map_underline(val: &str) -> Option<UnderlineStyle> {
    match val {
        "none" => None,
        "double" => Some(UnderlineStyle::Double),
        "thick" | "thickDash" | "thickDotDash" | "thickDotDotDash" | "thickDotted" => {
            Some(UnderlineStyle::Thick)
        }
        "dotted" | "dottedHeavy" => Some(UnderlineStyle::Dotted),
        "dash" | "dashedHeavy" | "dashLong" | "dashLongHeavy" => Some(UnderlineStyle::Dash),
        "wave" | "wavyHeavy" | "wavyDouble" => Some(UnderlineStyle::Wave),
        _ => Some(UnderlineStyle::Single),
    }
}

/// Maps a `w:highlight @w:val` string to [`HighlightColor`].
fn map_highlight(val: &str) -> HighlightColor {
    match val {
        "black" => HighlightColor::Black,
        "blue" => HighlightColor::Blue,
        "cyan" => HighlightColor::Cyan,
        "darkBlue" => HighlightColor::DarkBlue,
        "darkCyan" => HighlightColor::DarkCyan,
        "darkGray" => HighlightColor::DarkGray,
        "darkGreen" => HighlightColor::DarkGreen,
        "darkMagenta" => HighlightColor::DarkMagenta,
        "darkRed" => HighlightColor::DarkRed,
        "darkYellow" => HighlightColor::DarkYellow,
        "green" => HighlightColor::Green,
        "lightGray" => HighlightColor::LightGray,
        "magenta" => HighlightColor::Magenta,
        "red" => HighlightColor::Red,
        "white" => HighlightColor::White,
        "yellow" => HighlightColor::Yellow,
        _ => HighlightColor::None,
    }
}

// ── Public mappers ────────────────────────────────────────────────────────────

/// Maps a [`DocxPPr`] (OOXML paragraph properties) to [`ParaProps`].
///
/// All measurements are converted from twips (1/20 pt) to points.
/// `outline_lvl` is shifted from 0-indexed (OOXML) to 1-indexed (model).
/// `num_id=0` is treated as "remove numbering" per ECMA-376 §17.9.25.
pub(crate) fn map_ppr(ppr: &DocxPPr) -> ParaProps {
    let mut props = ParaProps::default();

    props.alignment = ppr.jc.as_deref().map(map_jc);

    if let Some(ref ind) = ppr.ind {
        props.indent_start = ind.left.map(twips_to_pt);
        props.indent_end = ind.right.map(twips_to_pt);
        props.indent_first_line = ind.first_line.map(twips_to_pt);
        props.indent_hanging = ind.hanging.map(twips_to_pt);
    }

    if let Some(ref sp) = ppr.spacing {
        props.space_before = sp.before.map(|v| Spacing::Exact(twips_to_pt(v)));
        props.space_after = sp.after.map(|v| Spacing::Exact(twips_to_pt(v)));
        if let Some(line) = sp.line {
            props.line_height = Some(map_line_height(line, sp.line_rule.as_deref()));
        }
    }

    props.keep_together = ppr.keep_lines;
    props.keep_with_next = ppr.keep_next;
    props.page_break_before = ppr.page_break_before;
    props.bidi = ppr.bidi;

    // OOXML outline_lvl is 0-indexed; model is 1-indexed (None = body text).
    props.outline_level = ppr.outline_lvl.map(|l| l + 1);

    // Widow control: true/absent → 2 lines (Word default); false → 0 (disabled).
    props.widow_control = ppr.widow_control.map(|v| if v { 2u8 } else { 0u8 });

    // Numbering: num_id=0 means "explicitly remove numbering".
    if let Some(ref np) = ppr.num_pr {
        if np.num_id != 0 {
            props.list_id = Some(ListId::new(np.num_id.to_string()));
            props.list_level = Some(np.ilvl);
        }
    }

    props
}

/// Maps a [`DocxRPr`] (OOXML run properties) to [`CharProps`].
///
/// Font sizes are in half-points (`w:sz`); letter spacing in twips (`w:spacing`).
/// Both are converted to points. Toggle properties map directly as `Option<bool>`.
pub(crate) fn map_rpr(rpr: &DocxRPr) -> CharProps {
    let mut props = CharProps::default();

    props.bold = rpr.bold;
    props.italic = rpr.italic;
    props.small_caps = rpr.small_caps;
    props.all_caps = rpr.all_caps;
    props.shadow = rpr.shadow;

    // Double strikethrough takes precedence over single.
    props.strikethrough = match (rpr.dstrike, rpr.strike) {
        (Some(true), _) => Some(StrikethroughStyle::Double),
        (_, Some(true)) => Some(StrikethroughStyle::Single),
        _ => None,
    };

    props.underline = rpr.underline.as_deref().and_then(map_underline);

    props.color = rpr
        .color
        .as_deref()
        .and_then(hex_color)
        .map(DocumentColor::Rgb);

    props.highlight_color = rpr
        .highlight
        .as_deref()
        .map(map_highlight)
        .filter(|h| *h != HighlightColor::None);

    // w:sz and w:szCs are in half-points.
    props.font_size = rpr.sz.map(|hp| Points::new(hp as f64 / 2.0));
    props.font_size_complex = rpr.sz_cs.map(|hp| Points::new(hp as f64 / 2.0));

    if let Some(ref fonts) = rpr.fonts {
        props.font_name = fonts.ascii.clone().or_else(|| fonts.h_ansi.clone());
        props.font_name_complex = fonts.cs.clone();
        props.font_name_east_asian = fonts.east_asia.clone();
    }

    // w:kern threshold in half-points: 0 = off, >0 = enabled.
    props.kerning = rpr.kern.map(|k| k > 0);

    // w:spacing is in twips.
    props.letter_spacing = rpr.spacing.map(|sp| Points::new(sp as f64 / 20.0));

    // w:w is a percentage integer (100 = normal).
    props.scale = rpr.scale.map(|s| s as f32 / 100.0);

    props.language = rpr.lang.as_deref().map(LanguageTag::new);

    props.vertical_align = rpr.vert_align.as_deref().and_then(|v| match v {
        "superscript" => Some(VerticalAlign::Superscript),
        "subscript" => Some(VerticalAlign::Subscript),
        _ => None,
    });

    props
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::model::paragraph::{DocxInd, DocxNumPr, DocxSpacing};

    fn ppr_with_jc(jc: &str) -> DocxPPr {
        DocxPPr { jc: Some(jc.into()), ..Default::default() }
    }

    // ── map_ppr ──────────────────────────────────────────────────────────────

    #[test]
    fn twip_conversion_720() {
        let ppr = DocxPPr {
            ind: Some(DocxInd { left: Some(720), ..Default::default() }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        assert_eq!(props.indent_start.unwrap().value(), 36.0);
    }

    #[test]
    fn jc_both_maps_to_justify() {
        assert_eq!(
            map_ppr(&ppr_with_jc("both")).alignment,
            Some(ParagraphAlignment::Justify)
        );
    }

    #[test]
    fn jc_distribute_maps_to_justify() {
        assert_eq!(
            map_ppr(&ppr_with_jc("distribute")).alignment,
            Some(ParagraphAlignment::Justify)
        );
    }

    #[test]
    fn jc_center_maps_to_center() {
        assert_eq!(
            map_ppr(&ppr_with_jc("center")).alignment,
            Some(ParagraphAlignment::Center)
        );
    }

    #[test]
    fn line_auto_276_is_multiple_1_15() {
        let ppr = DocxPPr {
            spacing: Some(DocxSpacing {
                line: Some(276),
                line_rule: Some("auto".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        if let Some(LineHeight::Multiple(m)) = props.line_height {
            assert!((m - 1.15_f32).abs() < 0.001, "expected ~1.15, got {m}");
        } else {
            panic!("expected LineHeight::Multiple");
        }
    }

    #[test]
    fn line_exact_240_is_12pt() {
        let ppr = DocxPPr {
            spacing: Some(DocxSpacing {
                line: Some(240),
                line_rule: Some("exact".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        if let Some(LineHeight::Exact(pts)) = props.line_height {
            assert_eq!(pts.value(), 12.0);
        } else {
            panic!("expected LineHeight::Exact");
        }
    }

    #[test]
    fn outline_lvl_0_becomes_1() {
        let ppr = DocxPPr { outline_lvl: Some(0), ..Default::default() };
        assert_eq!(map_ppr(&ppr).outline_level, Some(1));
    }

    #[test]
    fn num_id_zero_is_none() {
        let ppr = DocxPPr {
            num_pr: Some(DocxNumPr { num_id: 0, ilvl: 0 }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        assert!(props.list_id.is_none());
    }

    #[test]
    fn num_id_3_ilvl_1() {
        let ppr = DocxPPr {
            num_pr: Some(DocxNumPr { num_id: 3, ilvl: 1 }),
            ..Default::default()
        };
        let props = map_ppr(&ppr);
        assert_eq!(props.list_id.as_ref().map(|l| l.as_str()), Some("3"));
        assert_eq!(props.list_level, Some(1));
    }

    // ── map_rpr ──────────────────────────────────────────────────────────────

    #[test]
    fn half_point_24_is_12pt() {
        let rpr = DocxRPr { sz: Some(24), ..Default::default() };
        let props = map_rpr(&rpr);
        assert_eq!(props.font_size.unwrap().value(), 12.0);
    }

    #[test]
    fn bold_none_is_none() {
        let rpr = DocxRPr { bold: None, ..Default::default() };
        assert!(map_rpr(&rpr).bold.is_none());
    }

    #[test]
    fn bold_some_true() {
        let rpr = DocxRPr { bold: Some(true), ..Default::default() };
        assert_eq!(map_rpr(&rpr).bold, Some(true));
    }

    #[test]
    fn bold_some_false() {
        let rpr = DocxRPr { bold: Some(false), ..Default::default() };
        assert_eq!(map_rpr(&rpr).bold, Some(false));
    }

    #[test]
    fn dstrike_takes_precedence_over_strike() {
        let rpr = DocxRPr {
            dstrike: Some(true),
            strike: Some(true),
            ..Default::default()
        };
        assert_eq!(map_rpr(&rpr).strikethrough, Some(StrikethroughStyle::Double));
    }
}
