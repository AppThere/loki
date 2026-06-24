// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Drop-cap (dropped initial / raised cap) paragraph property.
//!
//! A drop cap enlarges the first character(s) of a paragraph to span several
//! lines, with the body text wrapping around it. ODF `style:drop-cap` (inside
//! `style:paragraph-properties`); OOXML `w:framePr` with `w:dropCap` on the
//! framed initial paragraph.
//!
//! This type captures the *import* representation only — the layout engine does
//! not yet render dropped initials (the glyph is currently shown inline at body
//! size). Carrying the property losslessly is the prerequisite for that work.

use loki_primitives::units::Points;

/// How many leading characters of the paragraph are enlarged.
///
/// ODF `style:length` (`"word"` or an integer count); OOXML has no explicit
/// count — the framed paragraph's content is the dropped text, so importers
/// default to a single character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum DropCapLength {
    /// Enlarge the first `n` characters (`n >= 1`).
    Chars(u8),
    /// Enlarge the whole first word. ODF `style:length="word"`.
    Word,
}

impl Default for DropCapLength {
    fn default() -> Self {
        DropCapLength::Chars(1)
    }
}

/// A dropped/raised initial-capital specification.
///
/// ODF `style:drop-cap`; OOXML `w:framePr` (`w:dropCap`, `w:lines`, `w:hSpace`).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DropCap {
    /// Number of text lines the cap spans (the "drop"). OOXML `w:lines`;
    /// ODF `style:lines`. Always `>= 1`; `1` degenerates to an inline initial.
    pub lines: u8,
    /// How many leading characters are enlarged.
    pub length: DropCapLength,
    /// Distance between the cap and the wrapped body text.
    /// OOXML `w:hSpace`; ODF `style:distance`.
    pub distance: Points,
    /// `true` when the cap sits in the page margin (OOXML `w:dropCap="margin"`),
    /// `false` when it is dropped within the text body (`w:dropCap="drop"`,
    /// the only ODF mode).
    pub margin: bool,
}

impl Default for DropCap {
    fn default() -> Self {
        DropCap {
            lines: 1,
            length: DropCapLength::default(),
            distance: Points::new(0.0),
            margin: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_single_char_in_text() {
        let dc = DropCap::default();
        assert_eq!(dc.lines, 1);
        assert_eq!(dc.length, DropCapLength::Chars(1));
        assert!(!dc.margin);
    }

    #[test]
    fn length_default_is_one_char() {
        assert_eq!(DropCapLength::default(), DropCapLength::Chars(1));
    }
}
