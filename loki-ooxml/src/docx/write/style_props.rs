// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `<w:pPr>` / `<w:rPr>` property serializers shared by the styles writer and
//! the document-body writer.
//!
//! ECMA-376 §17.3 (Paragraph and run properties).

use quick_xml::Writer;

use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{LineHeight, ParaProps, Spacing};

use crate::docx::write::xml::{
    hex_color_val, pts_to_half_pts, pts_to_twips, write_empty, write_end, write_start, wval,
};

/// Writes a `<w:pPr>` element from [`ParaProps`] (nothing if no field is set).
pub(super) fn write_para_props_elem<W: std::io::Write>(w: &mut Writer<W>, pp: &ParaProps) {
    let has_ind = pp.indent_start.is_some()
        || pp.indent_end.is_some()
        || pp.indent_hanging.is_some()
        || pp.indent_first_line.is_some();
    let has_spacing =
        pp.space_before.is_some() || pp.space_after.is_some() || pp.line_height.is_some();
    let has_content =
        pp.alignment.is_some() || has_ind || has_spacing || pp.outline_level.is_some();
    if !has_content {
        return;
    }
    let _ = write_start(w, "w:pPr", &[]);

    if let Some(align) = pp.alignment {
        use loki_doc_model::style::props::para_props::ParagraphAlignment;
        let jc = match align {
            ParagraphAlignment::Right => "right",
            ParagraphAlignment::Center => "center",
            ParagraphAlignment::Justify => "both",
            _ => "left",
        };
        let _ = write_empty(w, "w:jc", &wval(jc));
    }

    if has_ind {
        write_indent_elem(w, pp);
    }
    if has_spacing {
        write_spacing_elem(w, pp);
    }

    if let Some(lvl) = pp.outline_level {
        // Model `outline_level` is 1-indexed (1 = Heading 1); OOXML
        // `w:outlineLvl` is 0-indexed. Mirror `map_ppr`'s `+1` on import.
        let lvl_s = lvl.saturating_sub(1).to_string();
        let _ = write_empty(w, "w:outlineLvl", &wval(&lvl_s));
    }

    let _ = write_end(w, "w:pPr");
}

/// Emits `<w:ind>` from the indentation fields (left / right / hanging / firstLine).
fn write_indent_elem<W: std::io::Write>(w: &mut Writer<W>, pp: &ParaProps) {
    let left = pp.indent_start.map_or(0, |v| pts_to_twips(v.value()));
    let right = pp.indent_end.map_or(0, |v| pts_to_twips(v.value()));
    let hanging = pp.indent_hanging.map_or(0, |v| pts_to_twips(v.value()));
    let first_line = pp.indent_first_line.map_or(0, |v| pts_to_twips(v.value()));
    let (left_s, right_s) = (left.to_string(), right.to_string());
    let (hanging_s, first_s) = (hanging.to_string(), first_line.to_string());
    let mut attrs: Vec<(&str, &str)> = Vec::new();
    if left != 0 {
        attrs.push(("w:left", &left_s));
    }
    if right != 0 {
        attrs.push(("w:right", &right_s));
    }
    // `w:hanging` and `w:firstLine` are mutually exclusive in OOXML; hanging
    // wins when both are present (it is the more specific list-style indent).
    if hanging != 0 {
        attrs.push(("w:hanging", &hanging_s));
    } else if first_line != 0 {
        attrs.push(("w:firstLine", &first_s));
    }
    if !attrs.is_empty() {
        let _ = write_empty(w, "w:ind", &attrs);
    }
}

/// Emits `<w:spacing>` merging before / after points and the line rule.
// Line values are small, bounded document measurements: the f32→i32 cast after
// rounding cannot realistically truncate or change sign.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn write_spacing_elem<W: std::io::Write>(w: &mut Writer<W>, pp: &ParaProps) {
    let before = pp.space_before.map(spacing_twips);
    let after = pp.space_after.map(spacing_twips);
    // Map LineHeight back to (w:line, w:lineRule). `Multiple` is a ratio
    // (1.5 = 1.5×) stored in 240ths; Exact/AtLeast are twips. Mirrors the
    // reader's `map_line_height` (line/240.0 for auto).
    let line_rule: Option<(i32, Option<&str>)> = pp.line_height.and_then(|lh| match lh {
        LineHeight::Multiple(m) => Some(((m * 240.0).round() as i32, None)),
        LineHeight::Exact(pt) => Some((pts_to_twips(pt.value()), Some("exact"))),
        LineHeight::AtLeast(pt) => Some((pts_to_twips(pt.value()), Some("atLeast"))),
        // `LineHeight` is #[non_exhaustive]; unknown variants emit no line rule.
        _ => None,
    });

    let before_s = before.unwrap_or(0).to_string();
    let after_s = after.unwrap_or(0).to_string();
    let line_s = line_rule.map(|(l, _)| l.to_string());
    let mut attrs: Vec<(&str, &str)> = Vec::new();
    if before.is_some() {
        attrs.push(("w:before", &before_s));
    }
    if after.is_some() {
        attrs.push(("w:after", &after_s));
    }
    if let (Some((_, rule)), Some(line_s)) = (line_rule, &line_s) {
        attrs.push(("w:line", line_s));
        if let Some(rule) = rule {
            attrs.push(("w:lineRule", rule));
        }
    }
    if !attrs.is_empty() {
        let _ = write_empty(w, "w:spacing", &attrs);
    }
}

/// Converts a [`Spacing`] to twips (percent spacing is not representable here).
fn spacing_twips(s: Spacing) -> i32 {
    match s {
        Spacing::Exact(pt) => pts_to_twips(pt.value()),
        _ => 0,
    }
}

/// Writes a `<w:rPr>` element from [`CharProps`] (nothing if no field is set).
pub(super) fn write_char_props_elem<W: std::io::Write>(w: &mut Writer<W>, cp: &CharProps) {
    let has_content = cp.bold.is_some()
        || cp.italic.is_some()
        || cp.underline.is_some()
        || cp.strikethrough.is_some()
        || cp.font_size.is_some()
        || cp.font_name.is_some()
        || cp.color.is_some()
        || cp.background_color.is_some()
        || cp.small_caps.is_some()
        || cp.vertical_align.is_some();
    if !has_content {
        return;
    }
    let _ = write_start(w, "w:rPr", &[]);
    emit_char_props(w, cp);
    let _ = write_end(w, "w:rPr");
}

/// Emits `<w:rPr>` child elements for the given [`CharProps`] without the
/// wrapping `<w:rPr>` tags (so callers can embed additional children).
pub(crate) fn emit_char_props<W: std::io::Write>(w: &mut Writer<W>, cp: &CharProps) {
    if let Some(ref font) = cp.font_name {
        let _ = write_empty(
            w,
            "w:rFonts",
            &[("w:ascii", font.as_str()), ("w:hAnsi", font.as_str())],
        );
    }
    if cp.bold == Some(true) {
        let _ = write_empty(w, "w:b", &[]);
    }
    if cp.italic == Some(true) {
        let _ = write_empty(w, "w:i", &[]);
    }
    if cp.small_caps == Some(true) {
        let _ = write_empty(w, "w:smallCaps", &[]);
    }
    if let Some(ref ul) = cp.underline {
        use loki_doc_model::style::props::char_props::UnderlineStyle;
        let v = match ul {
            UnderlineStyle::Double => "double",
            UnderlineStyle::Dotted => "dotted",
            UnderlineStyle::Dash => "dash",
            UnderlineStyle::Wave => "wave",
            UnderlineStyle::Thick => "thick",
            _ => "single",
        };
        let _ = write_empty(w, "w:u", &wval(v));
    }
    if let Some(ref st) = cp.strikethrough {
        use loki_doc_model::style::props::char_props::StrikethroughStyle;
        match st {
            StrikethroughStyle::Double => {
                let _ = write_empty(w, "w:dstrike", &[]);
            }
            _ => {
                let _ = write_empty(w, "w:strike", &[]);
            }
        }
    }
    if let Some(ref va) = cp.vertical_align {
        use loki_doc_model::style::props::char_props::VerticalAlign;
        let v = match va {
            VerticalAlign::Superscript => "superscript",
            VerticalAlign::Subscript => "subscript",
            _ => "baseline",
        };
        let _ = write_empty(w, "w:vertAlign", &wval(v));
    }
    if let Some(ref color) = cp.color
        && let Some(hex) = color.to_hex()
    {
        let hex_val = hex_color_val(&hex);
        let _ = write_empty(w, "w:color", &wval(&hex_val));
    }
    if let Some(pt) = cp.font_size {
        let half = pts_to_half_pts(pt.value()).to_string();
        let _ = write_empty(w, "w:sz", &wval(&half));
        let _ = write_empty(w, "w:szCs", &wval(&half));
    }
}
