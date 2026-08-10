// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Built-in default list styles (usage audit §10 tier 1).
//!
//! The ribbon's bullet/numbered buttons need a `ListStyle` to point at when
//! the document has none. These two constructors produce Word's canonical
//! defaults — the full nine levels, so Tab promotion never walks off the end
//! of the definition:
//!
//! - **Bullet**: the •/○/▪ glyph cycle (already normalised to real Unicode —
//!   the importer's PUA remapping produces the same three glyphs, so a
//!   round-trip through DOCX is stable).
//! - **Numbered**: the `1.` / `a.` / `i.` scheme cycle, each level restarting
//!   at 1 and showing only its own counter.
//!
//! Geometry matches Word's list defaults: content indented 36 pt per level
//! with an 18 pt hanging label.
//!
//! The ids are namespaced (`__default-*`) like the importer's `__DocDefault`
//! so they cannot collide with a document's own named list styles; `ensure_*`
//! helpers insert them into a catalog only when absent, so a document that
//! already carries a style under this id (a round-tripped export) keeps its
//! own definition.

use loki_primitives::units::Points;

use super::catalog::StyleCatalog;
use super::list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};

/// Id of the built-in bullet list style.
pub const DEFAULT_BULLET_LIST_ID: &str = "__default-bullet";
/// Id of the built-in numbered list style.
pub const DEFAULT_NUMBERED_LIST_ID: &str = "__default-numbered";

/// Number of levels each default defines (the model's maximum).
const LEVELS: u8 = 9;
/// Content indent per level, in points (Word: 720 twips = 36 pt per level).
const INDENT_STEP_PT: f64 = 36.0;
/// Hanging label width, in points (Word: 360 twips = 18 pt).
const HANGING_PT: f64 = 18.0;

/// Geometry shared by both defaults for `level`.
fn level_geometry(level: u8) -> (Points, Points) {
    let indent = Points::new(INDENT_STEP_PT * f64::from(level + 1));
    (indent, Points::new(HANGING_PT))
}

/// Word's default bullet glyph cycle: level 0 •, level 1 ○, level 2 ▪, repeat.
fn bullet_char(level: u8) -> char {
    match level % 3 {
        0 => '•',
        1 => '○',
        _ => '▪',
    }
}

/// Word's default numbering-scheme cycle: decimal, lower alpha, lower roman.
fn numbering_scheme(level: u8) -> NumberingScheme {
    match level % 3 {
        0 => NumberingScheme::Decimal,
        1 => NumberingScheme::LowerAlpha,
        _ => NumberingScheme::LowerRoman,
    }
}

/// The built-in nine-level bullet list style.
#[must_use]
pub fn default_bullet_list_style() -> ListStyle {
    let levels = (0..LEVELS)
        .map(|level| {
            let (indent_start, hanging_indent) = level_geometry(level);
            ListLevel {
                level,
                kind: ListLevelKind::Bullet {
                    char: BulletChar::Char(bullet_char(level)),
                    font: None,
                },
                indent_start,
                hanging_indent,
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: None,
                char_props: Default::default(),
            }
        })
        .collect();
    ListStyle {
        id: ListId::new(DEFAULT_BULLET_LIST_ID),
        display_name: None,
        levels,
        extensions: Default::default(),
    }
}

/// The built-in nine-level numbered list style.
#[must_use]
pub fn default_numbered_list_style() -> ListStyle {
    let levels = (0..LEVELS)
        .map(|level| {
            let (indent_start, hanging_indent) = level_geometry(level);
            ListLevel {
                level,
                kind: ListLevelKind::Numbered {
                    scheme: numbering_scheme(level),
                    start_value: 1,
                    format: format!("%{}.", level + 1),
                    display_levels: 1,
                },
                indent_start,
                hanging_indent,
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: None,
                char_props: Default::default(),
            }
        })
        .collect();
    ListStyle {
        id: ListId::new(DEFAULT_NUMBERED_LIST_ID),
        display_name: None,
        levels,
        extensions: Default::default(),
    }
}

/// Inserts the default bullet style into `catalog` if absent; returns its id.
/// A document already carrying a style under this id keeps its own definition.
pub fn ensure_default_bullet(catalog: &mut StyleCatalog) -> ListId {
    let id = ListId::new(DEFAULT_BULLET_LIST_ID);
    catalog
        .list_styles
        .entry(id.clone())
        .or_insert_with(default_bullet_list_style);
    id
}

/// Inserts the default numbered style into `catalog` if absent; returns its id.
pub fn ensure_default_numbered(catalog: &mut StyleCatalog) -> ListId {
    let id = ListId::new(DEFAULT_NUMBERED_LIST_ID);
    catalog
        .list_styles
        .entry(id.clone())
        .or_insert_with(default_numbered_list_style);
    id
}

#[cfg(test)]
#[path = "list_defaults_tests.rs"]
mod tests;
