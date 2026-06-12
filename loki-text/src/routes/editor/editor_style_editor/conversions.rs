// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Conversion helpers between [`ParagraphStyle`] and [`StyleDraft`].

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::UnderlineStyle;
use loki_doc_model::style::props::para_props::{ParagraphAlignment, Spacing};
use loki_doc_model::style::props::{CharProps, ParaProps};

use super::super::editor_state::StyleDraft;

/// Converts a catalog `ParagraphStyle` to an editable `StyleDraft`.
pub(crate) fn style_to_draft(style: &ParagraphStyle) -> StyleDraft {
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
        alignment: match style.para_props.alignment {
            Some(ParagraphAlignment::Center) => "Center",
            Some(ParagraphAlignment::Right) => "Right",
            Some(ParagraphAlignment::Justify) => "Justify",
            _ => "Left",
        }
        .to_string(),
        font_size_str: style
            .char_props
            .font_size
            .map(|s| format!("{:.0}", s.value()))
            .unwrap_or_default(),
        bold: style.char_props.bold.unwrap_or(false),
        italic: style.char_props.italic.unwrap_or(false),
        underline: style.char_props.underline.is_some(),
        space_before_str: match style.para_props.space_before {
            Some(Spacing::Exact(pt)) => format!("{:.0}", pt.value()),
            _ => String::new(),
        },
        space_after_str: match style.para_props.space_after {
            Some(Spacing::Exact(pt)) => format!("{:.0}", pt.value()),
            _ => String::new(),
        },
        indent_first_str: style
            .para_props
            .indent_first_line
            .map(|pt| format!("{:.0}", pt.value()))
            .unwrap_or_default(),
        is_custom: style.is_custom,
    }
}

/// Converts a `StyleDraft` back to a `ParagraphStyle` for catalog storage.
pub(crate) fn draft_to_style(draft: &StyleDraft) -> ParagraphStyle {
    let alignment = match draft.alignment.as_str() {
        "Center" => Some(ParagraphAlignment::Center),
        "Right" => Some(ParagraphAlignment::Right),
        "Justify" => Some(ParagraphAlignment::Justify),
        "Left" => Some(ParagraphAlignment::Left),
        _ => None,
    };
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
            indent_first_line: draft
                .indent_first_str
                .trim()
                .parse::<f64>()
                .ok()
                .map(Points::new),
            ..Default::default()
        },
        char_props: CharProps {
            bold: Some(draft.bold),
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
