// SPDX-License-Identifier: Apache-2.0

//! The read-only inspector model for **list styles** (Spec 05 M6 — the list
//! family).
//!
//! Unlike the paragraph and character families, list styles do **not** inherit:
//! a [`ListStyle`](loki_doc_model::style::ListStyle) is a flat vector of
//! per-level definitions (ADR-0004; ODF `text:list-style` / OOXML
//! `w:abstractNum`), so there is no provenance chain to resolve — each level's
//! label kind and geometry are read directly. This module flattens a list style
//! into one display row per indent level for the family panel, mirroring the
//! `paragraph_inspector_rows` / `character_inspector_rows` role for its family.
//!
//! Pure + i18n-free (like the sibling inspector models): value-like text such as
//! `"Bullet"`, `"Decimal"`, or `"18 pt"` is baked here the same way the character
//! inspector bakes `"On"` / `"12 pt"`; the family panel localises the surrounding
//! field labels.

use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, NumberingScheme, StyleCatalog,
};

/// One display row per list-style indent level (Spec 05 §9 list family).
///
/// A list style is non-inheriting, so every field is the level's own value —
/// there is no provenance to attach.
pub struct ListLevelRow {
    /// The zero-indexed indent level (0 = outermost).
    pub level: u8,
    /// The label kind — a bullet char, a numbered scheme, or "None".
    pub label: String,
    /// The start indent, formatted in points.
    pub indent: String,
    /// The hanging indent, formatted in points.
    pub hanging: String,
    /// The label alignment (Left / Right / Center).
    pub alignment: String,
}

/// Builds one [`ListLevelRow`] per indent level of the list style `id`, in level
/// order. Returns an empty vector when `id` is not a list style in the catalog.
#[must_use]
pub fn list_inspector_rows(catalog: &StyleCatalog, id: &ListId) -> Vec<ListLevelRow> {
    let Some(style) = catalog.list_styles.get(id) else {
        return Vec::new();
    };
    style.levels.iter().map(build_row).collect()
}

fn build_row(level: &ListLevel) -> ListLevelRow {
    ListLevelRow {
        level: level.level,
        label: label_display(&level.kind),
        indent: fmt_points(level.indent_start),
        hanging: fmt_points(level.hanging_indent),
        alignment: alignment_display(level.label_alignment),
    }
}

/// A short human-readable summary of a level's label.
fn label_display(kind: &ListLevelKind) -> String {
    match kind {
        ListLevelKind::Bullet { char, .. } => match char {
            BulletChar::Char(c) => format!("Bullet {c}"),
            BulletChar::Image { .. } => "Image bullet".to_string(),
            _ => "Bullet".to_string(),
        },
        ListLevelKind::Numbered { scheme, .. } => format!("Numbered · {}", scheme_name(*scheme)),
        ListLevelKind::None => "None".to_string(),
        _ => "—".to_string(),
    }
}

/// A short example/name for a numbering scheme.
fn scheme_name(scheme: NumberingScheme) -> &'static str {
    match scheme {
        NumberingScheme::Decimal => "Decimal",
        NumberingScheme::LowerAlpha => "a, b, c",
        NumberingScheme::UpperAlpha => "A, B, C",
        NumberingScheme::LowerRoman => "i, ii, iii",
        NumberingScheme::UpperRoman => "I, II, III",
        NumberingScheme::Ordinal => "Ordinal",
        NumberingScheme::None => "None",
        _ => "—",
    }
}

fn alignment_display(alignment: LabelAlignment) -> String {
    match alignment {
        LabelAlignment::Left => "Left",
        LabelAlignment::Right => "Right",
        LabelAlignment::Center => "Center",
        _ => "—",
    }
    .to_string()
}

fn fmt_points(p: Points) -> String {
    format!("{:.0} pt", p.value())
}

#[cfg(test)]
#[path = "style_list_inspector_tests.rs"]
mod tests;
