// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Character-level formatting properties.
//!
//! [`CharProps`] is derived directly from TR 29166 §6.2.1 "Text formatting"
//! feature table. Every property in that table is represented here.
//! ODF maps these to `style:text-properties`; OOXML maps them to `w:rPr`.

use loki_primitives::units::Points;
use loki_primitives::color::DocumentColor;
use crate::meta::LanguageTag;
use crate::content::attr::ExtensionBag;

/// The style of underline decoration on a text run.
///
/// TR 29166 §6.2.1. ODF `style:text-underline-style`; OOXML `w:u`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum UnderlineStyle {
    /// A single solid underline.
    Single,
    /// A double underline.
    Double,
    /// A dotted underline.
    Dotted,
    /// A dashed underline.
    Dash,
    /// A wavy underline.
    Wave,
    /// A thick solid underline.
    Thick,
}

/// The style of strikethrough decoration on a text run.
///
/// TR 29166 §6.2.1. ODF `style:text-line-through-style`;
/// OOXML `w:strike` / `w:dstrike`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum StrikethroughStyle {
    /// A single strikethrough line.
    Single,
    /// A double strikethrough line.
    Double,
}

/// Vertical text positioning for super/subscript.
///
/// TR 29166 §6.2.1. ODF `style:text-position`; OOXML `w:vertAlign`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum VerticalAlign {
    /// Text is raised above the baseline (superscript).
    Superscript,
    /// Text is lowered below the baseline (subscript).
    Subscript,
    /// Text is at the normal baseline (explicit reset).
    Baseline,
}

/// A fixed highlight color applied to text background.
///
/// TR 29166 §6.2.1. ODF `fo:background-color` (named colors only);
/// OOXML `w:highlight` with a fixed color name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum HighlightColor {
    Yellow,
    Green,
    Cyan,
    Magenta,
    Blue,
    Red,
    DarkBlue,
    DarkCyan,
    DarkGreen,
    DarkMagenta,
    DarkRed,
    DarkYellow,
    DarkGray,
    LightGray,
    Black,
    White,
    None,
}

/// Character-level formatting properties.
///
/// Derived from TR 29166 §6.2.1 "Text formatting" feature table.
/// ODF maps to `style:text-properties`. OOXML maps to `w:rPr`.
/// All fields are `Option<T>` — `None` means "inherit from style/default".
/// See ADR-0003 for the rationale behind the Option pattern.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharProps {
    // ── Font ──────────────────────────────────────────────────────────────

    /// The primary font family name. ODF `style:font-name`;
    /// OOXML `w:rFonts w:ascii` / `w:hAnsi`.
    pub font_name: Option<String>,

    /// Font for complex script (BiDi) text. ODF `style:font-name-complex`;
    /// OOXML `w:rFonts w:cs`.
    pub font_name_complex: Option<String>,

    /// Font for East Asian text. ODF `style:font-name-asian`;
    /// OOXML `w:rFonts w:eastAsia`.
    pub font_name_east_asian: Option<String>,

    /// Font size in points. ODF `fo:font-size`; OOXML `w:sz` (half-points).
    pub font_size: Option<Points>,

    /// Font size for complex script. ODF `style:font-size-complex`;
    /// OOXML `w:szCs`.
    pub font_size_complex: Option<Points>,

    // ── Style flags ───────────────────────────────────────────────────────

    /// Bold. ODF `fo:font-weight bold`; OOXML `w:b`.
    pub bold: Option<bool>,

    /// Italic. ODF `fo:font-style italic`; OOXML `w:i`.
    pub italic: Option<bool>,

    /// Underline style. ODF `style:text-underline-style`; OOXML `w:u`.
    pub underline: Option<UnderlineStyle>,

    /// Strikethrough style. ODF `style:text-line-through-style`;
    /// OOXML `w:strike` / `w:dstrike`.
    pub strikethrough: Option<StrikethroughStyle>,

    /// Outline (hollow) text effect. ODF `style:text-outline`; OOXML `w:outline`.
    pub outline: Option<bool>,

    /// Shadow text effect. ODF `fo:text-shadow`; OOXML `w:shadow`.
    pub shadow: Option<bool>,

    /// Small caps. ODF `fo:font-variant small-caps`; OOXML `w:smallCaps`.
    pub small_caps: Option<bool>,

    /// All caps. ODF `fo:text-transform uppercase`; OOXML `w:caps`.
    pub all_caps: Option<bool>,

    /// Vertical alignment (super/subscript). ODF `style:text-position`;
    /// OOXML `w:vertAlign`.
    pub vertical_align: Option<VerticalAlign>,

    // ── Color ─────────────────────────────────────────────────────────────

    /// Foreground (text) color. ODF `fo:color`; OOXML `w:color`.
    pub color: Option<DocumentColor>,

    /// Background color behind the run. ODF `fo:background-color`;
    /// OOXML `w:shd`.
    pub background_color: Option<DocumentColor>,

    /// Named highlight color (limited palette). ODF `fo:background-color`
    /// (named); OOXML `w:highlight`.
    pub highlight_color: Option<HighlightColor>,

    // ── Spacing ───────────────────────────────────────────────────────────

    /// Letter spacing (tracking) in points. ODF `fo:letter-spacing`;
    /// OOXML `w:spacing`.
    pub letter_spacing: Option<Points>,

    /// Word spacing in points. ODF `style:word-spacing`; no direct OOXML equiv.
    pub word_spacing: Option<Points>,

    /// Kerning enabled. ODF `style:letter-kerning`; OOXML `w:kern`.
    pub kerning: Option<bool>,

    /// Horizontal text scaling as a percentage (100.0 = normal).
    /// ODF `style:text-scale`; OOXML `w:w`.
    pub scale: Option<f32>,

    // ── Language ──────────────────────────────────────────────────────────

    /// Language for spell-check and hyphenation. ODF `fo:language` +
    /// `fo:country`; OOXML `w:lang w:val`. TR 29166 §6.2.6.
    pub language: Option<LanguageTag>,

    /// Language for complex-script text. ODF `style:language-complex`;
    /// OOXML `w:lang w:bidi`.
    pub language_complex: Option<LanguageTag>,

    /// Language for East Asian text. ODF `style:language-asian`;
    /// OOXML `w:lang w:eastAsia`.
    pub language_east_asian: Option<LanguageTag>,

    // ── Links ─────────────────────────────────────────────────────────────

    /// Hyperlink URL if this run is rendered as a hyperlink.
    /// ODF: `text:a href`. OOXML: `w:hyperlink r:id`.
    pub hyperlink: Option<String>,

    // ── Extensions ────────────────────────────────────────────────────────

    /// Format-specific properties not representable in the above fields.
    pub extensions: ExtensionBag,
}

impl CharProps {
    /// Merges `parent` into `self`, filling in `None` fields from the parent.
    ///
    /// `self` (the child) wins for any field that is `Some`. Fields that
    /// are `None` in `self` are inherited from `parent`. This implements
    /// the style inheritance chain described in ADR-0003.
    #[must_use]
    pub fn merged_with_parent(mut self, parent: &CharProps) -> CharProps {
        macro_rules! inherit {
            ($field:ident) => {
                if self.$field.is_none() {
                    self.$field = parent.$field.clone();
                }
            };
        }
        inherit!(font_name);
        inherit!(font_name_complex);
        inherit!(font_name_east_asian);
        inherit!(font_size);
        inherit!(font_size_complex);
        inherit!(bold);
        inherit!(italic);
        inherit!(underline);
        inherit!(strikethrough);
        inherit!(outline);
        inherit!(shadow);
        inherit!(small_caps);
        inherit!(all_caps);
        inherit!(vertical_align);
        inherit!(color);
        inherit!(background_color);
        inherit!(highlight_color);
        inherit!(letter_spacing);
        inherit!(word_spacing);
        inherit!(kerning);
        inherit!(scale);
        inherit!(language);
        inherit!(language_complex);
        inherit!(language_east_asian);
        inherit!(hyperlink);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_all_none() {
        let cp = CharProps::default();
        assert!(cp.font_name.is_none());
        assert!(cp.bold.is_none());
        assert!(cp.color.is_none());
    }

    #[test]
    fn merge_child_wins_for_some() {
        let parent = CharProps {
            font_name: Some("Times New Roman".into()),
            bold: Some(false),
            font_size: Some(Points::new(12.0)),
            ..Default::default()
        };
        let child = CharProps {
            font_name: Some("Arial".into()),
            bold: Some(true),
            ..Default::default()
        };
        let merged = child.merged_with_parent(&parent);
        assert_eq!(merged.font_name.as_deref(), Some("Arial"));
        assert_eq!(merged.bold, Some(true));
        // font_size inherited from parent
        assert_eq!(merged.font_size, Some(Points::new(12.0)));
    }
}
