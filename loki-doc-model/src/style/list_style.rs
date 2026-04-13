// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! List style definitions.
//!
//! TR 29166 §7.2.5 describes the ODF and OOXML list models. ODF uses
//! `text:list-style`; OOXML uses `w:abstractNum` / `w:num`. This module
//! provides the abstract representation used by both.
//!
//! See ADR-0004 for the two-level list model design decision.

use loki_primitives::units::Points;
use crate::content::attr::ExtensionBag;
use crate::style::props::char_props::CharProps;

/// Unique identifier for a list style definition.
///
/// Paragraph properties reference this id to indicate that a paragraph
/// participates in a given list. TR 29166 §7.2.5.
/// ODF: `text:style-name` on `text:list-style`. OOXML: `w:abstractNumId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListId(pub String);

impl ListId {
    /// Creates a new [`ListId`] from the given string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the list id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ListId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A single bullet character for unordered lists.
///
/// ODF `text:bullet-char`; OOXML `w:lvlText w:val` with a single character.
/// TR 29166 §7.2.5.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum BulletChar {
    /// A Unicode bullet character (e.g. `'•'`, `'◦'`, `'▪'`).
    Char(char),
    /// An image bullet (stored opaquely; image data is in the extension bag).
    Image,
}

/// The numbering scheme for an ordered list level.
///
/// TR 29166 §6.2.5. ODF `text:num-format`; OOXML `w:numFmt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum NumberingScheme {
    /// Arabic numerals: 1, 2, 3, …
    Decimal,
    /// Lowercase letters: a, b, c, …
    LowerAlpha,
    /// Uppercase letters: A, B, C, …
    UpperAlpha,
    /// Lowercase Roman numerals: i, ii, iii, …
    LowerRoman,
    /// Uppercase Roman numerals: I, II, III, …
    UpperRoman,
    /// Chicago-style ordinals (1st, 2nd, 3rd). OOXML `ordinal`.
    Ordinal,
    /// No visible label (used for continuation lists).
    None,
}

/// The alignment of the list label relative to the list indent.
///
/// ODF `text:label-followed-by`; OOXML `w:lvlJc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum LabelAlignment {
    /// The label is left-aligned within its indent box (the default).
    #[default]
    Left,
    /// The label is right-aligned within its indent box.
    Right,
    /// The label is centered within its indent box.
    Center,
}

/// The content and rendering of a list level's label.
///
/// TR 29166 §7.2.5.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ListLevelKind {
    /// An unordered (bullet) level.
    Bullet {
        /// The bullet character.
        char: BulletChar,
        /// Override font for the bullet character, if different from the
        /// paragraph font (e.g. `"Symbol"`, `"Wingdings"`).
        font: Option<String>,
    },
    /// An ordered (numbered) level.
    Numbered {
        /// The numbering scheme.
        scheme: NumberingScheme,
        /// The starting counter value (usually 1).
        start_value: u32,
        /// Format string for the label. `%1` is replaced by the counter
        /// at level 1, `%2` at level 2, etc. ODF `text:num-format` +
        /// `text:num-suffix`; OOXML `w:lvlText w:val`.
        format: String,
        /// How many preceding levels to display in the label.
        /// 1 = only this level; 2 = "1.2"; etc.
        display_levels: u8,
    },
    /// A list level with no visible label.
    None,
}

/// A single indent level within a list style.
///
/// TR 29166 §7.2.5. ODF: one `text:list-level-style-*` element.
/// OOXML: one `w:lvl` element inside `w:abstractNum`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListLevel {
    /// The zero-indexed level number (0 = outermost).
    pub level: u8,
    /// The label kind (bullet or numbered).
    pub kind: ListLevelKind,
    /// Indentation from the start edge to the list item content.
    /// ODF `text:space-before`; OOXML `w:ind w:left`.
    pub indent_start: Points,
    /// The hanging indent — how far the label sticks out to the left of
    /// the content start. ODF `text:min-label-width`; OOXML `w:ind w:hanging`.
    pub hanging_indent: Points,
    /// The alignment of the label text within the label box.
    pub label_alignment: LabelAlignment,
    /// An explicit tab stop after the label, if present.
    /// ODF `text:list-tab-stop-position`; OOXML `w:tabStop` inside `w:lvl`.
    pub tab_stop_after_label: Option<Points>,
    /// Character properties for the label character (not the item content).
    pub char_props: CharProps,
}

/// A named list style defining up to 9 indent levels.
///
/// TR 29166 §7.2.5. ODF: `text:list-style`. OOXML: `w:abstractNum`.
/// See ADR-0004 for the two-level list model design decision.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ListStyle {
    /// The unique identifier for this list style.
    pub id: ListId,
    /// An optional human-readable display name.
    pub display_name: Option<String>,
    /// Level definitions, one per indent level, 0-indexed (max 9 levels).
    pub levels: Vec<ListLevel>,
    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_id_round_trip() {
        let id = ListId::new("list-1");
        assert_eq!(id.as_str(), "list-1");
        assert_eq!(id.to_string(), "list-1");
    }

    #[test]
    fn list_style_levels_vec() {
        let style = ListStyle {
            id: ListId::new("ls1"),
            display_name: None,
            levels: vec![],
            extensions: ExtensionBag::default(),
        };
        assert!(style.levels.is_empty());
    }
}
