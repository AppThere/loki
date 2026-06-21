// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph property mapping (`OdfParaProps` → `ParaProps`) and its border helper.

use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_doc_model::style::props::para_props::{LineHeight, ParaProps, Spacing};
use loki_doc_model::style::props::tab_stop::TabStop;
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use crate::odt::model::styles::OdfParaProps;
use crate::xml_util::parse_length;

use super::cell::{map_tab_stop, map_text_align};

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
            out.indent_hanging = Some(loki_primitives::units::Points::new(-v));
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
    if let Some(pts) = props.line_height_at_least.as_deref().and_then(parse_length) {
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

    // ── Borders ────────────────────────────────────────────────────────────
    // ODF fo:border is a CSS shorthand "width style color"; per-side values
    // override the shorthand on a per-side basis.
    let border_fallback = props.border.as_deref().and_then(parse_odf_border);
    out.border_top = props
        .border_top
        .as_deref()
        .and_then(parse_odf_border)
        .or_else(|| border_fallback.clone());
    out.border_bottom = props
        .border_bottom
        .as_deref()
        .and_then(parse_odf_border)
        .or_else(|| border_fallback.clone());
    out.border_left = props
        .border_left
        .as_deref()
        .and_then(parse_odf_border)
        .or_else(|| border_fallback.clone());
    out.border_right = props
        .border_right
        .as_deref()
        .and_then(parse_odf_border)
        .or(border_fallback);

    // ── Padding ────────────────────────────────────────────────────────────
    // ODF only has fo:padding shorthand; apply it to all four sides.
    if let Some(pts) = props.padding.as_deref().and_then(parse_length) {
        out.padding_top = Some(pts);
        out.padding_bottom = Some(pts);
        out.padding_left = Some(pts);
        out.padding_right = Some(pts);
    }

    // ── Background color ───────────────────────────────────────────────────
    if let Some(hex) = props.background_color.as_deref()
        && hex != "transparent"
        && let Ok(dc) = DocumentColor::from_hex(hex)
    {
        out.background_color = Some(dc);
    }

    // ── Bidirectional direction ────────────────────────────────────────────
    // ODF `style:writing-mode` values that indicate RTL text direction.
    // "rl-tb" is right-to-left, top-to-bottom (Arabic/Hebrew).
    // "rl" is shorthand for rl-tb.
    if matches!(
        props.writing_mode.as_deref(),
        Some("rl-tb" | "rl" | "tb-rl")
    ) {
        out.bidi = Some(true);
    }

    // ── Tab stops ──────────────────────────────────────────────────────────
    if !props.tab_stops.is_empty() {
        let stops: Vec<TabStop> = props.tab_stops.iter().filter_map(map_tab_stop).collect();
        if !stops.is_empty() {
            out.tab_stops = Some(stops);
        }
    }

    out
}

/// Parse an ODF CSS-like border shorthand `"width style color"` into a
/// [`Border`].
///
/// ODF `fo:border` uses the XSL-FO shorthand syntax, e.g. `"1pt solid #000000"`.
/// Width tokens are parsed via [`parse_length`]; style is mapped to
/// [`BorderStyle`]; colour is parsed as a `#RRGGBB` hex string.
/// Returns `None` when the string is `"none"` or cannot be parsed.
pub(super) fn parse_odf_border(s: &str) -> Option<Border> {
    let s = s.trim();
    if s == "none" || s.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = s.split_whitespace().collect();
    let mut width: Option<Points> = None;
    let mut style = BorderStyle::Solid;
    let mut color: Option<DocumentColor> = None;

    for tok in &tokens {
        if width.is_none()
            && let Some(pts) = parse_length(tok)
        {
            width = Some(pts);
            continue;
        }
        match *tok {
            "none" => return None,
            "solid" => style = BorderStyle::Solid,
            "dashed" => style = BorderStyle::Dashed,
            "dotted" => style = BorderStyle::Dotted,
            "double" => style = BorderStyle::Double,
            "groove" => style = BorderStyle::Groove,
            "ridge" => style = BorderStyle::Ridge,
            "inset" => style = BorderStyle::Inset,
            "outset" => style = BorderStyle::Outset,
            "wave" => style = BorderStyle::Wave,
            hex if hex.starts_with('#') => {
                if let Ok(dc) = DocumentColor::from_hex(hex) {
                    color = Some(dc);
                }
            }
            _ => {}
        }
    }

    let width = width.unwrap_or_else(|| Points::new(1.0));
    Some(Border {
        style,
        width,
        color,
        spacing: None,
    })
}
