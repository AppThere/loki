// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table-cell property mapping and tab-stop / alignment helpers.

use loki_doc_model::content::table::row::{CellProps, CellTextDirection, CellVerticalAlign};
use loki_doc_model::style::props::para_props::ParagraphAlignment;
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader, TabStop};
use loki_primitives::color::DocumentColor;

use crate::odt::model::styles::{OdfCellProps, OdfTabStop};
use crate::xml_util::parse_length;

use super::paragraph::parse_odf_border;

// ── Cell property mapping ──────────────────────────────────────────────────────

/// Convert [`OdfCellProps`] to the format-neutral [`CellProps`].
///
/// All length strings are parsed via [`parse_length`]. Unparseable or absent
/// values silently map to `None`. ODF 1.3 §17.18.
///
/// NOTE: ODF cell properties are mapped to the same [`CellProps`] type as
/// OOXML. The layout engine applies them identically.
pub(crate) fn map_cell_props(cell_props: &OdfCellProps) -> CellProps {
    CellProps {
        padding_top: cell_props.padding_top.as_deref().and_then(parse_length),
        padding_bottom: cell_props.padding_bottom.as_deref().and_then(parse_length),
        padding_left: cell_props.padding_left.as_deref().and_then(parse_length),
        padding_right: cell_props.padding_right.as_deref().and_then(parse_length),
        vertical_align: cell_props
            .vertical_align
            .as_deref()
            .and_then(map_odf_vertical_align),
        text_direction: cell_props
            .writing_mode
            .as_deref()
            .and_then(map_odf_writing_mode),
        background_color: cell_props.background_color.as_deref().and_then(|c| {
            if c == "transparent" {
                None
            } else {
                DocumentColor::from_hex(c).ok()
            }
        }),
        border_top: cell_props.border_top.as_deref().and_then(parse_odf_border),
        border_bottom: cell_props
            .border_bottom
            .as_deref()
            .and_then(parse_odf_border),
        border_left: cell_props.border_left.as_deref().and_then(parse_odf_border),
        border_right: cell_props
            .border_right
            .as_deref()
            .and_then(parse_odf_border),
    }
}

/// Map an ODF `style:vertical-align` string to [`CellVerticalAlign`].
///
/// `"automatic"` falls through to the default `Top`.
pub(crate) fn map_odf_vertical_align(val: &str) -> Option<CellVerticalAlign> {
    match val {
        "top" | "automatic" => Some(CellVerticalAlign::Top),
        "middle" => Some(CellVerticalAlign::Middle),
        "bottom" => Some(CellVerticalAlign::Bottom),
        _ => None,
    }
}

/// Map an ODF `style:writing-mode` string to [`CellTextDirection`].
pub(crate) fn map_odf_writing_mode(val: &str) -> Option<CellTextDirection> {
    match val {
        "lr-tb" | "lr" => Some(CellTextDirection::LrTb),
        "tb-rl" | "tb" => Some(CellTextDirection::TbRl),
        "tb-lr" => Some(CellTextDirection::TbLr),
        "bt-lr" => Some(CellTextDirection::BtLr),
        _ => None,
    }
}

/// Map an [`OdfTabStop`] to a doc-model [`TabStop`].
///
/// ODF tab alignment values: `"left"` → [`TabAlignment::Left`],
/// `"right"` → [`TabAlignment::Right`], `"center"` → [`TabAlignment::Center`],
/// `"char"` → [`TabAlignment::Decimal`].
/// `style:leader-style` is now mapped to [`TabLeader`].
pub(super) fn map_tab_stop(ts: &OdfTabStop) -> Option<TabStop> {
    let position = parse_length(&ts.position)?;
    let alignment = match ts.tab_type.as_deref() {
        Some("right") => TabAlignment::Right,
        Some("center") => TabAlignment::Center,
        Some("char") => TabAlignment::Decimal,
        _ => TabAlignment::Left,
    };
    Some(TabStop {
        position,
        alignment,
        leader: map_leader_style(ts.leader_style.as_deref()),
    })
}

fn map_leader_style(s: Option<&str>) -> TabLeader {
    match s {
        Some("dotted") => TabLeader::Dot,
        Some("dash" | "long-dash" | "dot-dash" | "dot-dot-dash") => TabLeader::Dash,
        Some("solid" | "wave" | "small-wave" | "double-wave") => TabLeader::Underscore,
        Some(
            "bold" | "bold-dash" | "bold-long-dash" | "bold-dot-dash" | "bold-dot-dot-dash"
            | "bold-wave",
        ) => TabLeader::Heavy,
        _ => TabLeader::None,
    }
}

/// Map an ODF `fo:text-align` string to [`ParagraphAlignment`].
pub(super) fn map_text_align(s: &str) -> ParagraphAlignment {
    match s {
        "right" | "end" => ParagraphAlignment::Right,
        "center" => ParagraphAlignment::Center,
        "justify" | "both" => ParagraphAlignment::Justify,
        _ => ParagraphAlignment::Left,
    }
}
