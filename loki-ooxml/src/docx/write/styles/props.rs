// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `<w:pPr>` and `<w:rPr>` element serializers for paragraph and character props.

use quick_xml::Writer;

use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;

use crate::docx::write::xml::{
    hex_color_val, pts_to_half_pts, pts_to_twips, write_empty, write_end, write_start, wval,
};

/// Writes a `<w:pPr>` element from [`ParaProps`] (empty if nothing set).
pub(super) fn write_para_props_elem<W: std::io::Write>(w: &mut Writer<W>, pp: &ParaProps) {
    let has_content = pp.alignment.is_some()
        || pp.indent_start.is_some()
        || pp.indent_end.is_some()
        || pp.indent_hanging.is_some()
        || pp.space_before.is_some()
        || pp.space_after.is_some();
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

    let has_ind =
        pp.indent_start.is_some() || pp.indent_end.is_some() || pp.indent_hanging.is_some();
    if has_ind {
        let left = pp.indent_start.map_or(0, |v| pts_to_twips(v.value()));
        let right = pp.indent_end.map_or(0, |v| pts_to_twips(v.value()));
        let hanging = pp.indent_hanging.map_or(0, |v| pts_to_twips(v.value()));
        let left_s = left.to_string();
        let right_s = right.to_string();
        let hanging_s = hanging.to_string();
        let mut ind_attrs: Vec<(&str, &str)> = Vec::new();
        if left != 0 {
            ind_attrs.push(("w:left", &left_s));
        }
        if right != 0 {
            ind_attrs.push(("w:right", &right_s));
        }
        if hanging != 0 {
            ind_attrs.push(("w:hanging", &hanging_s));
        }
        if !ind_attrs.is_empty() {
            let _ = write_empty(w, "w:ind", &ind_attrs);
        }
    }

    if pp.space_before.is_some() || pp.space_after.is_some() {
        use loki_doc_model::style::props::para_props::Spacing;
        let before = pp.space_before.map_or(0, |v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        });
        let after = pp.space_after.map_or(0, |v| match v {
            Spacing::Exact(pt) => pts_to_twips(pt.value()),
            Spacing::Percent(_) | _ => 0,
        });
        let before_s = before.to_string();
        let after_s = after.to_string();
        let _ = write_empty(
            w,
            "w:spacing",
            &[("w:before", &before_s), ("w:after", &after_s)],
        );
    }

    let _ = write_end(w, "w:pPr");
}

/// Writes a `<w:rPr>` element from [`CharProps`] (empty if nothing set).
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
