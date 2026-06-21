// SPDX-License-Identifier: Apache-2.0

//! Conversions between a catalog [`ParagraphStyle`] and the editable
//! [`StyleDraft`] bound to the style editor's form inputs.

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::UnderlineStyle;
use loki_doc_model::style::props::para_props::{LineHeight, ParagraphAlignment, Spacing};
use loki_doc_model::style::props::{CharProps, ParaProps};

use super::super::editor_state::StyleDraft;

/// Formats an optional point measurement as a whole-number string (empty for `None`).
fn points_str(pt: Option<Points>) -> String {
    pt.map(|p| format!("{:.0}", p.value())).unwrap_or_default()
}

/// Parses a points field; blank or invalid input becomes `None` (inherit/unset).
fn parse_points(s: &str) -> Option<Points> {
    s.trim().parse::<f64>().ok().map(Points::new)
}

/// Converts a catalog `ParagraphStyle` to an editable `StyleDraft`.
pub(crate) fn style_to_draft(style: &ParagraphStyle) -> StyleDraft {
    let cp = &style.char_props;
    let pp = &style.para_props;
    StyleDraft {
        id: style.id.as_str().to_string(),
        name: style
            .display_name
            .clone()
            .unwrap_or_else(|| style.id.as_str().to_string()),
        parent: style
            .parent
            .as_ref()
            .map(|p| p.as_str().to_string())
            .unwrap_or_default(),
        next: style.next_style_id.clone().unwrap_or_default(),
        alignment: match pp.alignment {
            Some(ParagraphAlignment::Center) => "Center",
            Some(ParagraphAlignment::Right) => "Right",
            Some(ParagraphAlignment::Justify) => "Justify",
            _ => "Left",
        }
        .to_string(),
        font_name: cp.font_name.clone().unwrap_or_default(),
        font_size_str: cp
            .font_size
            .map(|s| format!("{:.0}", s.value()))
            .unwrap_or_default(),
        font_weight: cp
            .font_weight
            .unwrap_or(if cp.bold == Some(true) { 700 } else { 400 }),
        italic: cp.italic.unwrap_or(false),
        underline: cp.underline.is_some(),
        space_before_str: match pp.space_before {
            Some(Spacing::Exact(pt)) => format!("{:.0}", pt.value()),
            _ => String::new(),
        },
        space_after_str: match pp.space_after {
            Some(Spacing::Exact(pt)) => format!("{:.0}", pt.value()),
            _ => String::new(),
        },
        line_height_str: match pp.line_height {
            Some(LineHeight::Multiple(pct)) => format!("{:.2}", pct / 100.0),
            _ => String::new(),
        },
        indent_start_str: points_str(pp.indent_start),
        indent_end_str: points_str(pp.indent_end),
        indent_first_str: points_str(pp.indent_first_line),
        indent_hanging_str: points_str(pp.indent_hanging),
        is_custom: style.is_custom,
    }
}

/// Converts a `StyleDraft` back to a `ParagraphStyle` for catalog storage.
///
/// `font_weight` is the source of truth; `bold` is derived from it (≥ 600 ⇒
/// bold) so a DOCX round-trip — which has no numeric weight — still collapses
/// to the right boolean. A weight of exactly 400 stores as `None` (inherit /
/// regular) so the style does not pin every run to Regular.
pub(crate) fn draft_to_style(draft: &StyleDraft) -> ParagraphStyle {
    let alignment = match draft.alignment.as_str() {
        "Center" => Some(ParagraphAlignment::Center),
        "Right" => Some(ParagraphAlignment::Right),
        "Justify" => Some(ParagraphAlignment::Justify),
        "Left" => Some(ParagraphAlignment::Left),
        _ => None,
    };
    let line_height = draft
        .line_height_str
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|&m| m > 0.0)
        .map(|m| LineHeight::Multiple(m * 100.0));
    ParagraphStyle {
        id: StyleId::new(&draft.id),
        display_name: if draft.name.is_empty() {
            None
        } else {
            Some(draft.name.clone())
        },
        parent: if draft.parent.is_empty() {
            None
        } else {
            Some(StyleId::new(&draft.parent))
        },
        linked_char_style: None,
        next_style_id: if draft.next.is_empty() {
            None
        } else {
            Some(draft.next.clone())
        },
        para_props: ParaProps {
            alignment,
            indent_start: parse_points(&draft.indent_start_str),
            indent_end: parse_points(&draft.indent_end_str),
            indent_first_line: parse_points(&draft.indent_first_str),
            indent_hanging: parse_points(&draft.indent_hanging_str),
            space_before: draft
                .space_before_str
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|&s| s >= 0.0)
                .map(|s| Spacing::Exact(Points::new(s))),
            space_after: draft
                .space_after_str
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|&s| s >= 0.0)
                .map(|s| Spacing::Exact(Points::new(s))),
            line_height,
            ..Default::default()
        },
        char_props: CharProps {
            font_name: if draft.font_name.trim().is_empty() {
                None
            } else {
                Some(draft.font_name.trim().to_string())
            },
            bold: Some(draft.font_weight >= 600),
            font_weight: if draft.font_weight == 400 {
                None
            } else {
                Some(draft.font_weight)
            },
            italic: Some(draft.italic),
            underline: if draft.underline {
                Some(UnderlineStyle::Single)
            } else {
                None
            },
            font_size: draft
                .font_size_str
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|&s| s > 0.0)
                .map(Points::new),
            ..Default::default()
        },
        is_default: false,
        is_custom: draft.is_custom,
        extensions: ExtensionBag::default(),
    }
}
