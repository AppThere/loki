// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Codec helpers: string encode/decode functions shared between the Loro
//! bridge write path (`write.rs`) and read path (`props_read.rs`, `inlines.rs`).
//!
//! Split from `inlines.rs` and `read.rs` to keep individual files under the
//! 300-line ceiling.

use crate::style::props::border::{Border, BorderStyle};
use crate::style::props::char_props::{
    HighlightColor, StrikethroughStyle, UnderlineStyle, VerticalAlign,
};
use crate::style::props::para_props::{LineHeight, ParagraphAlignment, Spacing};
use crate::style::props::tab_stop::{TabAlignment, TabLeader, TabStop};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

// ── CharProps decoders ────────────────────────────────────────────────────────

pub(super) fn decode_underline(s: &str) -> Option<UnderlineStyle> {
    match s {
        "Single" => Some(UnderlineStyle::Single),
        "Double" => Some(UnderlineStyle::Double),
        "Dotted" => Some(UnderlineStyle::Dotted),
        "Dash" => Some(UnderlineStyle::Dash),
        "Wave" => Some(UnderlineStyle::Wave),
        "Thick" => Some(UnderlineStyle::Thick),
        _ => None,
    }
}

pub(super) fn decode_strikethrough(s: &str) -> Option<StrikethroughStyle> {
    match s {
        "Single" => Some(StrikethroughStyle::Single),
        "Double" => Some(StrikethroughStyle::Double),
        _ => None,
    }
}

pub(super) fn decode_vertical_align(s: &str) -> Option<VerticalAlign> {
    match s {
        "Superscript" => Some(VerticalAlign::Superscript),
        "Subscript" => Some(VerticalAlign::Subscript),
        "Baseline" => Some(VerticalAlign::Baseline),
        _ => None,
    }
}

pub(super) fn decode_highlight_color(s: &str) -> Option<HighlightColor> {
    match s {
        // Explicit "no highlight" (e.g. highlight removed by the user) —
        // distinct from an absent mark, which means "inherit".
        "None" => Some(HighlightColor::None),
        "Black" => Some(HighlightColor::Black),
        "Blue" => Some(HighlightColor::Blue),
        "Cyan" => Some(HighlightColor::Cyan),
        "DarkBlue" => Some(HighlightColor::DarkBlue),
        "DarkCyan" => Some(HighlightColor::DarkCyan),
        "DarkGray" => Some(HighlightColor::DarkGray),
        "DarkGreen" => Some(HighlightColor::DarkGreen),
        "DarkMagenta" => Some(HighlightColor::DarkMagenta),
        "DarkRed" => Some(HighlightColor::DarkRed),
        "DarkYellow" => Some(HighlightColor::DarkYellow),
        "Green" => Some(HighlightColor::Green),
        "LightGray" => Some(HighlightColor::LightGray),
        "Magenta" => Some(HighlightColor::Magenta),
        "Red" => Some(HighlightColor::Red),
        "White" => Some(HighlightColor::White),
        "Yellow" => Some(HighlightColor::Yellow),
        _ => None,
    }
}

// ── ParaProps encoders ────────────────────────────────────────────────────────

pub(super) fn encode_alignment(a: &ParagraphAlignment) -> &'static str {
    match a {
        ParagraphAlignment::Left => "Left",
        ParagraphAlignment::Right => "Right",
        ParagraphAlignment::Center => "Center",
        ParagraphAlignment::Justify => "Justify",
        ParagraphAlignment::Distribute => "Distribute",
    }
}

pub(super) fn encode_spacing(s: &Spacing) -> String {
    match s {
        Spacing::Exact(pts) => format!("Exact:{}", pts.value()),
        Spacing::Percent(pct) => format!("Percent:{pct}"),
    }
}

pub(super) fn encode_line_height(lh: &LineHeight) -> String {
    match lh {
        LineHeight::Exact(pts) => format!("Exact:{}", pts.value()),
        LineHeight::AtLeast(pts) => format!("AtLeast:{}", pts.value()),
        LineHeight::Multiple(m) => format!("Multiple:{m}"),
    }
}

// ── ParaProps decoders ────────────────────────────────────────────────────────

pub(super) fn decode_alignment(s: &str) -> Option<ParagraphAlignment> {
    match s {
        "Left" => Some(ParagraphAlignment::Left),
        "Right" => Some(ParagraphAlignment::Right),
        "Center" => Some(ParagraphAlignment::Center),
        "Justify" => Some(ParagraphAlignment::Justify),
        "Distribute" => Some(ParagraphAlignment::Distribute),
        _ => None,
    }
}

pub(super) fn decode_spacing(s: &str) -> Option<Spacing> {
    if let Some(rest) = s.strip_prefix("Exact:") {
        rest.parse::<f64>()
            .ok()
            .map(|v| Spacing::Exact(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("Percent:") {
        rest.parse::<f32>().ok().map(Spacing::Percent)
    } else {
        None
    }
}

pub(super) fn decode_line_height(s: &str) -> Option<LineHeight> {
    if let Some(rest) = s.strip_prefix("Exact:") {
        rest.parse::<f64>()
            .ok()
            .map(|v| LineHeight::Exact(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("AtLeast:") {
        rest.parse::<f64>()
            .ok()
            .map(|v| LineHeight::AtLeast(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("Multiple:") {
        rest.parse::<f32>().ok().map(LineHeight::Multiple)
    } else {
        None
    }
}

// ── Tab-stop codec ───────────────────────────────────────────────────────────

/// Encode a tab-stop list as `"position_pt:Alignment:Leader"` entries joined
/// by `';'` (e.g. `"36:Left:None;144:Decimal:Dot"`).
pub(super) fn encode_tab_stops(stops: &[TabStop]) -> String {
    stops
        .iter()
        .map(|ts| {
            let alignment = match ts.alignment {
                TabAlignment::Left => "Left",
                TabAlignment::Right => "Right",
                TabAlignment::Center => "Center",
                TabAlignment::Decimal => "Decimal",
                TabAlignment::Clear => "Clear",
            };
            let leader = match ts.leader {
                TabLeader::None => "None",
                TabLeader::Dot => "Dot",
                TabLeader::Dash => "Dash",
                TabLeader::Underscore => "Underscore",
                TabLeader::Heavy => "Heavy",
                TabLeader::MiddleDot => "MiddleDot",
            };
            format!("{}:{}:{}", ts.position.value(), alignment, leader)
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Decode a tab-stop list produced by [`encode_tab_stops`]. Returns `None`
/// for unparseable input (including the pre-codec Debug-string encoding),
/// which callers treat as an absent property.
pub(super) fn decode_tab_stops(s: &str) -> Option<Vec<TabStop>> {
    if s.is_empty() {
        return Some(Vec::new());
    }
    let mut stops = Vec::new();
    for entry in s.split(';') {
        let mut fields = entry.splitn(3, ':');
        let position: f64 = fields.next()?.parse().ok()?;
        let alignment = match fields.next()? {
            "Left" => TabAlignment::Left,
            "Right" => TabAlignment::Right,
            "Center" => TabAlignment::Center,
            "Decimal" => TabAlignment::Decimal,
            "Clear" => TabAlignment::Clear,
            _ => return None,
        };
        let leader = match fields.next()? {
            "None" => TabLeader::None,
            "Dot" => TabLeader::Dot,
            "Dash" => TabLeader::Dash,
            "Underscore" => TabLeader::Underscore,
            "Heavy" => TabLeader::Heavy,
            "MiddleDot" => TabLeader::MiddleDot,
            _ => return None,
        };
        stops.push(TabStop {
            position: Points::new(position),
            alignment,
            leader,
        });
    }
    Some(stops)
}

// ── Border codec ─────────────────────────────────────────────────────────────

/// Encode a [`Border`] as a compact `"Style:width_pt:color:spacing_pt"` string.
///
/// TODO(loro-bridge): non-Rgb border colors (Theme/Cmyk) collapse to `auto`
/// here — the colon-delimited format cannot carry the `color_codec` encodings
/// (they contain `:`), so lifting this needs a format migration.
pub(super) fn encode_border(b: &Border) -> String {
    let style = match b.style {
        BorderStyle::None => "None",
        BorderStyle::Solid => "Solid",
        BorderStyle::Dashed => "Dashed",
        BorderStyle::Dotted => "Dotted",
        BorderStyle::Double => "Double",
        BorderStyle::Groove => "Groove",
        BorderStyle::Ridge => "Ridge",
        BorderStyle::Inset => "Inset",
        BorderStyle::Outset => "Outset",
        BorderStyle::Wave => "Wave",
    };
    let color = b
        .color
        .as_ref()
        .and_then(|c| c.to_hex())
        .unwrap_or_else(|| "auto".to_string());
    let spacing = b.spacing.map_or(0.0, |s| s.value());
    format!("{}:{}:{}:{}", style, b.width.value(), color, spacing)
}

/// Decode a border string produced by [`encode_border`].
pub(super) fn decode_border(s: &str) -> Option<Border> {
    let mut parts = s.splitn(4, ':');
    let style_str = parts.next()?;
    let width_str = parts.next()?;
    let color_str = parts.next()?;
    let spacing_str = parts.next()?;

    let style = match style_str {
        "None" => BorderStyle::None,
        "Solid" => BorderStyle::Solid,
        "Dashed" => BorderStyle::Dashed,
        "Dotted" => BorderStyle::Dotted,
        "Double" => BorderStyle::Double,
        "Groove" => BorderStyle::Groove,
        "Ridge" => BorderStyle::Ridge,
        "Inset" => BorderStyle::Inset,
        "Outset" => BorderStyle::Outset,
        "Wave" => BorderStyle::Wave,
        _ => return None,
    };
    let width: f64 = width_str.parse().ok()?;
    let color = if color_str == "auto" {
        None
    } else {
        DocumentColor::from_hex(color_str).ok()
    };
    let spacing: f64 = spacing_str.parse().ok()?;
    Some(Border {
        style,
        width: Points::new(width),
        color,
        spacing: if spacing == 0.0 {
            None
        } else {
            Some(Points::new(spacing))
        },
    })
}
