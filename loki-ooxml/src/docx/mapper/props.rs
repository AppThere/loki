// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph and character property mappers.
//!
//! Converts [`DocxPPr`] → [`ParaProps`] and [`DocxRPr`] → [`CharProps`].
//! All OOXML measurements are in twentieths of a point (twips); all model
//! measurements are in [`loki_primitives::units::Points`].

use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_doc_model::style::props::char_props::{HighlightColor, UnderlineStyle};
use loki_doc_model::style::props::drop_cap::{DropCap, DropCapLength};
use loki_doc_model::style::props::para_props::{
    LineHeight, ParaProps, ParagraphAlignment, Spacing,
};
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader, TabStop};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use crate::docx::model::paragraph::{DocxBorderEdge, DocxFramePr, DocxPPr};
use crate::xml_util::{hex_color, resolve_shading};

#[path = "props_rpr.rs"]
mod rpr;
pub(crate) use rpr::map_rpr;

// ── Internal conversion helpers ───────────────────────────────────────────────

/// Maps `w:framePr` to a [`DropCap`], or `None` when no drop cap is requested
/// (`w:dropCap` absent/`"none"`/`"default"`); length defaults to one character.
fn map_frame_pr(fp: &DocxFramePr) -> Option<DropCap> {
    let margin = match fp.drop_cap.as_deref()? {
        "drop" => false,
        "margin" => true,
        _ => return None,
    };
    Some(DropCap {
        lines: fp.lines.unwrap_or(1).max(1),
        length: DropCapLength::Chars(1),
        distance: twips_to_pt(fp.h_space.unwrap_or(0)),
        margin,
    })
}

/// Converts a twips integer to [`Points`] (1 pt = 20 twips).
fn twips_to_pt(twips: i32) -> Points {
    Points::new(f64::from(twips) / 20.0)
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
        #[allow(clippy::cast_precision_loss)]
        // Precision loss acceptable: values represent document measurements
        _ => LineHeight::Multiple(line as f32 / 240.0),
    }
}

/// Maps a `w:u @w:val` string to [`UnderlineStyle`] (`None` removes underline).
pub(super) fn map_underline(val: &str) -> Option<UnderlineStyle> {
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
pub(super) fn map_highlight(val: &str) -> HighlightColor {
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

/// Maps a `DocxBorderEdge` to a doc-model [`Border`].
///
/// `"nil"` and `"none"` produce [`BorderStyle::None`]. `@w:sz` is in eighths
/// of a point (ECMA-376 §17.3.4); `@w:space` is in points (not twips).
pub(crate) fn map_border_edge(edge: &DocxBorderEdge) -> Border {
    let style = match edge.val.as_str() {
        "nil" | "none" => BorderStyle::None,
        "double" => BorderStyle::Double,
        "dashed" | "dashSmallGap" | "dashDot" | "dashDotDot" | "dotDash" | "dotDotDash"
        | "dashDotStroked" => BorderStyle::Dashed,
        "dotted" | "dottedHeavy" => BorderStyle::Dotted,
        "wave" | "wavyHeavy" | "wavyDouble" => BorderStyle::Wave,
        _ => BorderStyle::Solid,
    };
    Border {
        style,
        width: Points::new(f64::from(edge.sz.unwrap_or(8)) / 8.0),
        color: edge
            .color
            .as_deref()
            .and_then(hex_color)
            .map(DocumentColor::Rgb),
        spacing: edge.space.map(|s| Points::new(f64::from(s))),
    }
}

// ── Public mappers ────────────────────────────────────────────────────────────

/// Maps a [`DocxPPr`] (OOXML paragraph properties) to [`ParaProps`].
///
/// All measurements are converted from twips (1/20 pt) to points.
/// `outline_lvl` is shifted from 0-indexed (OOXML) to 1-indexed (model).
/// `num_id=0` is treated as "remove numbering" per ECMA-376 §17.9.25.
pub(crate) fn map_ppr(ppr: &DocxPPr) -> ParaProps {
    let mut props = ParaProps {
        alignment: ppr.jc.as_deref().map(map_jc),
        ..Default::default()
    };

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

    // Drop cap (w:framePr w:dropCap). Only "drop"/"margin" produce a drop cap.
    if let Some(ref fp) = ppr.frame_pr {
        props.drop_cap = map_frame_pr(fp);
    }

    // OOXML outline_lvl is 0-indexed; model is 1-indexed (None = body text).
    props.outline_level = ppr.outline_lvl.map(|l| l + 1);

    // Widow control: true/absent → 2 lines (Word default); false → 0 (disabled).
    props.widow_control = ppr.widow_control.map(|v| if v { 2u8 } else { 0u8 });

    // Numbering: num_id=0 means "explicitly remove numbering".
    if let Some(ref np) = ppr.num_pr
        && np.num_id != 0
    {
        props.list_id = Some(ListId::new(np.num_id.to_string()));
        props.list_level = Some(np.ilvl);
    }

    // Paragraph borders (gap #6): w:pBdr → ParaProps border_* + padding_*.
    if let Some(ref pbdr) = ppr.p_bdr {
        props.border_top = pbdr.top.as_ref().map(map_border_edge);
        props.border_bottom = pbdr.bottom.as_ref().map(map_border_edge);
        props.border_left = pbdr.left.as_ref().map(map_border_edge);
        props.border_right = pbdr.right.as_ref().map(map_border_edge);
        props.border_between = pbdr.between.as_ref().map(map_border_edge);
        // w:space is in points (not twips) — use directly.
        props.padding_top = pbdr
            .top
            .as_ref()
            .and_then(|e| e.space)
            .map(|s| Points::new(f64::from(s)));
        props.padding_bottom = pbdr
            .bottom
            .as_ref()
            .and_then(|e| e.space)
            .map(|s| Points::new(f64::from(s)));
        props.padding_left = pbdr
            .left
            .as_ref()
            .and_then(|e| e.space)
            .map(|s| Points::new(f64::from(s)));
        props.padding_right = pbdr
            .right
            .as_ref()
            .and_then(|e| e.space)
            .map(|s| Points::new(f64::from(s)));
    }

    // Paragraph background from `w:shd`. Honours the pattern (`@w:val`): solid
    // fills, `pctN` blends of `@w:color` over `@w:fill`, and `clear` (fill only).
    if let Some(rgb) = resolve_shading(
        ppr.shd_fill.as_deref(),
        ppr.shd_val.as_deref(),
        ppr.shd_color.as_deref(),
    ) {
        props.background_color = Some(DocumentColor::Rgb(rgb));
    }

    // Tab stops: w:tabs → ParaProps.tab_stops.
    // "clear" entries remove inherited stops and are not forwarded as explicit stops.
    if !ppr.tabs.is_empty() {
        let stops: Vec<TabStop> = ppr
            .tabs
            .iter()
            .filter(|t| t.val != "clear")
            .map(|t| TabStop {
                position: twips_to_pt(t.pos),
                alignment: match t.val.as_str() {
                    "right" => TabAlignment::Right,
                    "center" => TabAlignment::Center,
                    "decimal" => TabAlignment::Decimal,
                    _ => TabAlignment::Left,
                },
                leader: match t.leader.as_deref() {
                    Some("dot") => TabLeader::Dot,
                    Some("hyphen" | "dash") => TabLeader::Dash,
                    Some("underscore") => TabLeader::Underscore,
                    Some("heavy") => TabLeader::Heavy,
                    Some("middleDot") => TabLeader::MiddleDot,
                    _ => TabLeader::None,
                },
            })
            .collect();
        if !stops.is_empty() {
            props.tab_stops = Some(stops);
        }
    }

    props
}
// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "props_tests.rs"]
mod tests;
