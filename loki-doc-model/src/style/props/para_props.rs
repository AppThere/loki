// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Paragraph-level formatting properties.
//!
//! [`ParaProps`] is derived directly from TR 29166 §6.2.2 "Paragraph
//! formatting" feature table. ODF maps these to
//! `style:paragraph-properties`; OOXML maps them to `w:pPr`.

use loki_primitives::units::Points;
use loki_primitives::color::DocumentColor;
use crate::content::attr::ExtensionBag;
use crate::style::props::border::Border;
use crate::style::props::tab_stop::TabStop;
use crate::style::list_style::ListId;

/// Horizontal text alignment within a paragraph.
///
/// TR 29166 §6.2.2. ODF `fo:text-align`; OOXML `w:jc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ParagraphAlignment {
    /// Left-aligned (the default for LTR text).
    #[default]
    Left,
    /// Right-aligned.
    Right,
    /// Centered.
    Center,
    /// Justified (both edges aligned).
    Justify,
    /// Distribute spacing evenly (Thai / East Asian justification).
    Distribute,
}

/// Paragraph spacing value — fixed points or a percentage of line height.
///
/// TR 29166 §6.2.2. ODF `fo:space-before` / `fo:space-after` (may be `%`);
/// OOXML `w:before` / `w:after` (twips) or `w:beforeLines` / `w:afterLines`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Spacing {
    /// An exact measurement in points.
    Exact(Points),
    /// A percentage of the line height (e.g. `100.0` = 100%).
    Percent(f32),
}

/// Line height specification.
///
/// TR 29166 §6.2.2. ODF `fo:line-height`; OOXML `w:line` + `w:lineRule`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum LineHeight {
    /// Exact line height in points (`w:lineRule exact`).
    Exact(Points),
    /// Minimum line height in points (`w:lineRule atLeast`).
    AtLeast(Points),
    /// A percentage multiple of the font size (e.g. `150.0` = 1.5×).
    /// ODF `fo:line-height` as `%`; OOXML `w:lineRule auto`.
    Multiple(f32),
}

/// Paragraph-level formatting properties.
///
/// Derived from TR 29166 §6.2.2 "Paragraph formatting" feature table.
/// ODF maps to `style:paragraph-properties`. OOXML maps to `w:pPr`.
/// All fields are `Option<T>` — `None` means "inherit from style/default".
/// See ADR-0003 for the rationale.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParaProps {
    // ── Alignment ─────────────────────────────────────────────────────────

    /// Horizontal text alignment. ODF `fo:text-align`; OOXML `w:jc`.
    pub alignment: Option<ParagraphAlignment>,

    // ── Indentation ───────────────────────────────────────────────────────

    /// Indentation from the start edge (left in LTR). ODF `fo:margin-left`;
    /// OOXML `w:ind w:left`.
    pub indent_start: Option<Points>,

    /// Indentation from the end edge (right in LTR). ODF `fo:margin-right`;
    /// OOXML `w:ind w:right`.
    pub indent_end: Option<Points>,

    /// Additional first-line indentation (positive = indent, negative = hanging
    /// when combined with `indent_start`). ODF `fo:text-indent`;
    /// OOXML `w:ind w:firstLine`.
    pub indent_first_line: Option<Points>,

    /// Hanging indent (the amount by which all lines except the first are
    /// indented relative to `indent_start`). ODF: expressed via negative
    /// `fo:text-indent`; OOXML `w:ind w:hanging`.
    pub indent_hanging: Option<Points>,

    // ── Spacing ───────────────────────────────────────────────────────────

    /// Space before the paragraph. ODF `fo:space-before`; OOXML `w:spacing w:before`.
    pub space_before: Option<Spacing>,

    /// Space after the paragraph. ODF `fo:space-after`; OOXML `w:spacing w:after`.
    pub space_after: Option<Spacing>,

    /// Line height. ODF `fo:line-height`; OOXML `w:spacing w:line` + `w:lineRule`.
    pub line_height: Option<LineHeight>,

    // ── Borders ───────────────────────────────────────────────────────────

    /// Top border. ODF `fo:border-top`; OOXML `w:pBdr/w:top`.
    pub border_top: Option<Border>,

    /// Bottom border. ODF `fo:border-bottom`; OOXML `w:pBdr/w:bottom`.
    pub border_bottom: Option<Border>,

    /// Start (left in LTR) border. ODF `fo:border-left`; OOXML `w:pBdr/w:left`.
    pub border_left: Option<Border>,

    /// End (right in LTR) border. ODF `fo:border-right`; OOXML `w:pBdr/w:right`.
    pub border_right: Option<Border>,

    /// Border between adjacent paragraphs sharing the same style.
    /// ODF `fo:border-*` with `style:join-border`; OOXML `w:pBdr/w:between`.
    pub border_between: Option<Border>,

    // ── Padding ───────────────────────────────────────────────────────────

    /// Padding inside the top border. ODF `fo:padding-top`; OOXML `w:pBdr/w:top w:space`.
    pub padding_top: Option<Points>,

    /// Padding inside the bottom border.
    pub padding_bottom: Option<Points>,

    /// Padding inside the start border.
    pub padding_left: Option<Points>,

    /// Padding inside the end border.
    pub padding_right: Option<Points>,

    // ── Background ────────────────────────────────────────────────────────

    /// Paragraph background fill color. ODF `fo:background-color`; OOXML `w:shd`.
    pub background_color: Option<DocumentColor>,

    // ── Tab stops ─────────────────────────────────────────────────────────

    /// Custom tab stops for this paragraph. ODF `style:tab-stop` list;
    /// OOXML `w:tabs`. TR 29166 §6.2.2.
    pub tab_stops: Option<Vec<TabStop>>,

    // ── Flow control ──────────────────────────────────────────────────────

    /// Prevent a page or column break within the paragraph.
    /// ODF `fo:keep-together`; OOXML `w:keepLines`.
    pub keep_together: Option<bool>,

    /// Prevent a page break between this paragraph and the next.
    /// ODF `fo:keep-with-next`; OOXML `w:keepNext`.
    pub keep_with_next: Option<bool>,

    /// Minimum number of lines at the bottom of a page (orphan control).
    /// ODF `fo:orphans`; OOXML `w:widowControl` (binary in OOXML).
    pub orphan_control: Option<u8>,

    /// Minimum number of lines at the top of a page (widow control).
    /// ODF `fo:widows`; OOXML `w:widowControl`.
    pub widow_control: Option<u8>,

    // ── Page breaks ───────────────────────────────────────────────────────

    /// Force a page break before this paragraph.
    /// ODF `fo:break-before page`; OOXML `w:pageBreakBefore`.
    pub page_break_before: Option<bool>,

    /// Force a page break after this paragraph (less common; usually
    /// achieved via `keep_with_next` on the preceding paragraph).
    pub page_break_after: Option<bool>,

    // ── List reference ────────────────────────────────────────────────────

    /// The list style this paragraph participates in.
    /// See ADR-0004 for the two-level list model rationale.
    /// ODF: `text:list-style-name`; OOXML: `w:numId`.
    pub list_id: Option<ListId>,

    /// The nesting level within the list (0-indexed). TR 29166 §7.2.5.
    pub list_level: Option<u8>,

    // ── Outline ───────────────────────────────────────────────────────────

    /// Heading outline level (1–9). `None` for body text.
    /// ODF `text:outline-level`; OOXML `w:outlineLvl`.
    pub outline_level: Option<u8>,

    // ── BiDi ──────────────────────────────────────────────────────────────

    /// Right-to-left paragraph direction.
    /// ODF `style:writing-mode`; OOXML `w:bidi`.
    pub bidi: Option<bool>,

    // ── Extensions ────────────────────────────────────────────────────────

    /// Format-specific properties not representable in the above fields.
    pub extensions: ExtensionBag,
}

impl ParaProps {
    /// Merges `parent` into `self`, filling in `None` fields from the parent.
    ///
    /// `self` (the child) wins for any field that is `Some`. See ADR-0003.
    #[must_use]
    pub fn merged_with_parent(mut self, parent: &ParaProps) -> ParaProps {
        macro_rules! inherit {
            ($field:ident) => {
                if self.$field.is_none() {
                    self.$field = parent.$field.clone();
                }
            };
        }
        inherit!(alignment);
        inherit!(indent_start);
        inherit!(indent_end);
        inherit!(indent_first_line);
        inherit!(indent_hanging);
        inherit!(space_before);
        inherit!(space_after);
        inherit!(line_height);
        inherit!(border_top);
        inherit!(border_bottom);
        inherit!(border_left);
        inherit!(border_right);
        inherit!(border_between);
        inherit!(padding_top);
        inherit!(padding_bottom);
        inherit!(padding_left);
        inherit!(padding_right);
        inherit!(background_color);
        inherit!(tab_stops);
        inherit!(keep_together);
        inherit!(keep_with_next);
        inherit!(orphan_control);
        inherit!(widow_control);
        inherit!(page_break_before);
        inherit!(page_break_after);
        inherit!(list_id);
        inherit!(list_level);
        inherit!(outline_level);
        inherit!(bidi);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_all_none() {
        let pp = ParaProps::default();
        assert!(pp.alignment.is_none());
        assert!(pp.indent_start.is_none());
    }

    #[test]
    fn merge_inherits_parent_alignment() {
        let parent = ParaProps {
            alignment: Some(ParagraphAlignment::Center),
            ..Default::default()
        };
        let child = ParaProps::default();
        let merged = child.merged_with_parent(&parent);
        assert_eq!(merged.alignment, Some(ParagraphAlignment::Center));
    }
}
