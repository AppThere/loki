// SPDX-License-Identifier: Apache-2.0

//! The editable draft for **one list-style level** (§10 tier 5). Pure and
//! panel-free, like the sibling draft modules: `from_level` snapshots a
//! catalog level into text-field-friendly strings, `to_level` parses them
//! back — `None` when a numeric field does not parse, so Apply declines
//! rather than writing a corrupted level. Fields the form does not expose
//! (label alignment, tab stop, label char props) pass through from the
//! existing level untouched.

use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::{BulletChar, ListLevel, ListLevelKind, NumberingScheme};

/// Which label family the level draft edits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::routes::editor) enum DraftKind {
    Bullet,
    Numbered,
}

/// One level of one list style, as the form edits it.
#[derive(Clone, PartialEq)]
pub(in crate::routes::editor) struct ListLevelDraft {
    /// The list style being edited.
    pub style_id: String,
    /// The level this draft addresses (0-indexed).
    pub level: u8,
    pub kind: DraftKind,
    /// The bullet character (first char is used; `•` when empty).
    pub bullet_char: String,
    pub scheme: NumberingScheme,
    /// The `%N`-style label format (e.g. `"%1."`).
    pub format: String,
    /// Start value, as typed.
    pub start: String,
    /// Content indent in points, as typed.
    pub indent: String,
    /// Hanging indent in points, as typed.
    pub hanging: String,
}

impl ListLevelDraft {
    /// Snapshots catalog `level` of `style_id` into a draft.
    pub(super) fn from_level(style_id: &str, level: &ListLevel) -> Self {
        let (kind, bullet_char, scheme, format, start) = match &level.kind {
            ListLevelKind::Numbered {
                scheme,
                start_value,
                format,
                ..
            } => (
                DraftKind::Numbered,
                String::new(),
                *scheme,
                format.clone(),
                start_value.to_string(),
            ),
            ListLevelKind::Bullet {
                char: BulletChar::Char(c),
                ..
            } => (
                DraftKind::Bullet,
                c.to_string(),
                NumberingScheme::Decimal,
                format!("%{}.", level.level + 1),
                "1".to_string(),
            ),
            // Image bullets and `None` levels edit as a plain bullet; the
            // original kind is replaced only on Apply.
            _ => (
                DraftKind::Bullet,
                "\u{2022}".to_string(),
                NumberingScheme::Decimal,
                format!("%{}.", level.level + 1),
                "1".to_string(),
            ),
        };
        Self {
            style_id: style_id.to_string(),
            level: level.level,
            kind,
            bullet_char,
            scheme,
            format,
            start,
            indent: format!("{:.0}", level.indent_start.value()),
            hanging: format!("{:.0}", level.hanging_indent.value()),
        }
    }

    /// Parses the draft back into a [`ListLevel`], carrying the unexposed
    /// fields over from `existing`. `None` when a numeric field is invalid.
    pub(super) fn to_level(&self, existing: &ListLevel) -> Option<ListLevel> {
        let indent: f64 = self.indent.trim().parse().ok()?;
        let hanging: f64 = self.hanging.trim().parse().ok()?;
        if !(0.0..=1000.0).contains(&indent) || !(0.0..=1000.0).contains(&hanging) {
            return None;
        }
        let kind = match self.kind {
            DraftKind::Bullet => ListLevelKind::Bullet {
                char: BulletChar::Char(self.bullet_char.chars().next().unwrap_or('\u{2022}')),
                font: match &existing.kind {
                    ListLevelKind::Bullet { font, .. } => font.clone(),
                    _ => None,
                },
            },
            DraftKind::Numbered => {
                let start: u32 = self.start.trim().parse().ok()?;
                let format = if self.format.trim().is_empty() {
                    format!("%{}.", self.level + 1)
                } else {
                    self.format.trim().to_string()
                };
                ListLevelKind::Numbered {
                    scheme: self.scheme,
                    start_value: start.max(1),
                    format,
                    display_levels: match &existing.kind {
                        ListLevelKind::Numbered { display_levels, .. } => *display_levels,
                        _ => 1,
                    },
                }
            }
        };
        Some(ListLevel {
            level: self.level,
            kind,
            indent_start: Points::new(indent),
            hanging_indent: Points::new(hanging),
            label_alignment: existing.label_alignment,
            tab_stop_after_label: existing.tab_stop_after_label,
            char_props: existing.char_props.clone(),
        })
    }
}

/// The schemes the form's selector offers, with their display names.
pub(super) const SCHEME_CHOICES: &[(NumberingScheme, &str)] = &[
    (NumberingScheme::Decimal, "1, 2, 3"),
    (NumberingScheme::LowerAlpha, "a, b, c"),
    (NumberingScheme::UpperAlpha, "A, B, C"),
    (NumberingScheme::LowerRoman, "i, ii, iii"),
    (NumberingScheme::UpperRoman, "I, II, III"),
];

#[cfg(test)]
#[path = "list_form_draft_tests.rs"]
mod tests;
