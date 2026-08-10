// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `<w:pPr>` paragraph-property serializers shared by the styles writer and the
//! document-body writer. Run (`<w:rPr>`) properties live in the sibling
//! [`run_props`](super::run_props) module.
//!
//! ECMA-376 §17.3 (Paragraph and run properties).

use quick_xml::Writer;

use loki_doc_model::style::props::para_props::{LineHeight, ParaProps, Spacing};
use loki_primitives::units::Points;

use crate::docx::write::xml::{
    hex_color_val, pts_to_twips, write_empty, write_end, write_start, wval,
};

/// Writes a `<w:pPr>` element from [`ParaProps`] (nothing if no field is set).
pub(super) fn write_para_props_elem<W: std::io::Write>(w: &mut Writer<W>, pp: &ParaProps) {
    let has_ind = pp.indent_start.is_some()
        || pp.indent_end.is_some()
        || pp.indent_hanging.is_some()
        || pp.indent_first_line.is_some();
    let has_spacing =
        pp.space_before.is_some() || pp.space_after.is_some() || pp.line_height.is_some();
    let has_borders = pp.border_top.is_some()
        || pp.border_bottom.is_some()
        || pp.border_left.is_some()
        || pp.border_right.is_some()
        || pp.border_between.is_some();
    let has_flags = pp.keep_together.is_some()
        || pp.keep_with_next.is_some()
        || pp.widow_control.is_some()
        || pp.page_break_before.is_some();
    let has_content = pp.alignment.is_some()
        || has_ind
        || has_spacing
        || pp.outline_level.is_some()
        || has_borders
        || has_flags
        || pp.background_color.is_some();
    if !has_content {
        return;
    }
    let _ = write_start(w, "w:pPr", &[]);

    // keep/widow/break toggles, the border box, and shading — the export
    // canonicalisation pass places these in CT_PPr order, so emit here.
    write_para_flags_borders_shading(w, pp);

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

/// Emits the `w:pPr` children both the styles and direct-props writers share but
/// historically dropped on DOCX export (importer parsed them, no writer wrote
/// them — the paragraph-level sibling of the `emit_char_props` symmetry): the
/// keep/widow/break `CT_OnOff` toggles, the paragraph border box (`w:pBdr`, five
/// edges each carrying its padding as `w:space`), and paragraph shading
/// (`w:shd`). Inverse of `map_ppr` (`docx/mapper/props.rs`). Child order is
/// normalised by the export canonicalisation pass, so these may be emitted
/// alongside the alignment/indent/spacing children in any order.
pub(super) fn write_para_flags_borders_shading<W: std::io::Write>(
    w: &mut Writer<W>,
    pp: &ParaProps,
) {
    write_para_flags(w, pp);
    write_para_borders_shading(w, pp);
}

/// The `CT_PPr` toggles that precede `w:numPr` in schema order (keep/break/
/// widow). Split from the borders half so `write_para_props_inline` can slot
/// `w:numPr` between them (§10 tier 2).
pub(super) fn write_para_flags<W: std::io::Write>(w: &mut Writer<W>, pp: &ParaProps) {
    write_on_off(w, "w:keepNext", pp.keep_with_next);
    write_on_off(w, "w:keepLines", pp.keep_together);
    write_on_off(w, "w:pageBreakBefore", pp.page_break_before);
    // widow_control is Option<u8> (map_ppr maps the OOXML bool → 2/0): a positive
    // value is on, 0 is an explicit off.
    match pp.widow_control {
        Some(0) => {
            let _ = write_empty(w, "w:widowControl", &wval("false"));
        }
        Some(_) => {
            let _ = write_empty(w, "w:widowControl", &wval("true"));
        }
        None => {}
    }
}

/// The paragraph border box and shading (follow `w:numPr` in `CT_PPr` order).
pub(super) fn write_para_borders_shading<W: std::io::Write>(w: &mut Writer<W>, pp: &ParaProps) {
    if pp.border_top.is_some()
        || pp.border_bottom.is_some()
        || pp.border_left.is_some()
        || pp.border_right.is_some()
        || pp.border_between.is_some()
    {
        let _ = write_start(w, "w:pBdr", &[]);
        write_pbdr_edge(w, "w:top", pp.border_top.as_ref(), pp.padding_top);
        write_pbdr_edge(w, "w:left", pp.border_left.as_ref(), pp.padding_left);
        write_pbdr_edge(w, "w:bottom", pp.border_bottom.as_ref(), pp.padding_bottom);
        write_pbdr_edge(w, "w:right", pp.border_right.as_ref(), pp.padding_right);
        // The inter-paragraph rule carries no padding of its own.
        write_pbdr_edge(w, "w:between", pp.border_between.as_ref(), None);
        let _ = write_end(w, "w:pBdr");
    }

    if let Some(ref bg) = pp.background_color
        && let Some(hex) = bg.to_hex()
    {
        // `w:val="clear"` + `@w:fill` round-trips through the reader's
        // `resolve_shading` (same shape as run/cell shading).
        let fill = hex_color_val(&hex);
        let _ = write_empty(w, "w:shd", &[("w:val", "clear"), ("w:fill", &fill)]);
    }
}

/// Emits a `CT_OnOff` paragraph toggle: `Some(true)` → bare element, `Some(false)`
/// → `@w:val="false"`, `None` → nothing.
fn write_on_off<W: std::io::Write>(w: &mut Writer<W>, tag: &str, v: Option<bool>) {
    match v {
        Some(true) => {
            let _ = write_empty(w, tag, &[]);
        }
        Some(false) => {
            let _ = write_empty(w, tag, &wval("false"));
        }
        None => {}
    }
}

/// One `w:pBdr` edge (`w:top`/`w:bottom`/`w:left`/`w:right`/`w:between`), the
/// inverse of `map_border_edge`: style → `@w:val`, width in points → eighth-pt
/// `@w:sz` (clamped 2..=96), padding in points → `@w:space`, colour → hex/auto.
/// A `BorderStyle::None` edge writes `@w:val="nil"`.
fn write_pbdr_edge<W: std::io::Write>(
    w: &mut Writer<W>,
    tag: &str,
    border: Option<&loki_doc_model::style::props::border::Border>,
    padding: Option<Points>,
) {
    use loki_doc_model::style::props::border::BorderStyle;
    let Some(b) = border else { return };
    if b.style == BorderStyle::None {
        let _ = write_empty(w, tag, &wval("nil"));
        return;
    }
    let val = match b.style {
        BorderStyle::Dashed => "dashed",
        BorderStyle::Dotted => "dotted",
        BorderStyle::Double => "double",
        BorderStyle::Inset => "inset",
        BorderStyle::Outset => "outset",
        BorderStyle::Wave => "wave",
        _ => "single",
    };
    #[allow(clippy::cast_possible_truncation)] // clamped to 2..=96 above the cast
    let sz = ((b.width.value() * 8.0).round().clamp(2.0, 96.0) as i32).to_string();
    // `w:space` is in points (the reader reads it back into padding directly).
    #[allow(clippy::cast_possible_truncation)] // bounded document measurement
    let space = padding
        .map_or(0, |p| p.value().round().clamp(0.0, 31.0) as i32)
        .to_string();
    let color = b
        .color
        .as_ref()
        .map_or_else(|| "auto".to_string(), crate::docx::write::xml::color_to_hex);
    let _ = write_empty(
        w,
        tag,
        &[
            ("w:val", val),
            ("w:sz", &sz),
            ("w:space", &space),
            ("w:color", &color),
        ],
    );
}
