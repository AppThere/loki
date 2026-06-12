// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph property mapper: [`DocxPPr`] → [`ParaProps`].

use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::para_props::{ParaProps, Spacing};
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader, TabStop};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use crate::docx::model::paragraph::DocxPPr;
use crate::xml_util::hex_color;

use super::border::map_border_edge;
use super::helpers::{map_jc, map_line_height, twips_to_pt};

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

    // Paragraph background from `w:shd @w:fill`.
    // "auto" means no fill; all other non-empty hex strings are mapped to an Rgb color.
    if let Some(ref hex) = ppr.shd_fill
        && hex != "auto"
        && let Some(rgb) = hex_color(hex)
    {
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
