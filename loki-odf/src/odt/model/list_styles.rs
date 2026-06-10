// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF list style model types.
//!
//! A `text:list-style` (ODF 1.3 §16.30) defines the formatting for each
//! nesting level of a list, covering both bullet and numbered variants.
//! Positioning uses either the legacy `text:space-before` model (ODF 1.1)
//! or the label-alignment model introduced in ODF 1.2.

use super::styles::OdfTextProps;

/// A named ODF list style. ODF 1.3 §16.30 `text:list-style`.
#[derive(Debug, Clone)]
pub(crate) struct OdfListStyle {
    /// `style:name` — identifier referenced by `text:style-name` attributes.
    pub name: String,
    /// Level definitions (0-indexed; level 0 corresponds to ODF level 1).
    pub levels: Vec<OdfListLevel>,
}

/// Formatting for a single nesting level of a list.
///
/// ODF 1.3 §16.31 `text:list-level-style-bullet`,
/// §16.33 `text:list-level-style-number`,
/// §16.34 `text:list-level-style-none`.
#[derive(Debug, Clone)]
pub(crate) struct OdfListLevel {
    /// 0-indexed level depth. Parsed from the 1-indexed `text:level`
    /// attribute: stored value = `text:level − 1`.
    pub level: u8,
    /// The kind of marker at this level.
    pub kind: OdfListLevelKind,

    // ── Legacy ODF 1.1 positioning (text:list-level-properties) ──────────
    /// `text:space-before` — indent from the left margin (ODF 1.1 model).
    pub legacy_space_before: Option<String>,
    /// `text:min-label-width` — minimum width of the label area (ODF 1.1).
    pub legacy_min_label_width: Option<String>,
    /// `text:min-label-distance` — gap between label and text (ODF 1.1).
    pub legacy_min_label_distance: Option<String>,

    // ── ODF 1.2+ label-alignment positioning ─────────────────────────────
    /// `text:label-followed-by` — separator after the label: `"listtab"`,
    /// `"space"`, or `"nothing"`. ODF 1.2+.
    pub label_followed_by: Option<String>,
    /// `text:list-tab-stop-position` — tab stop for label-alignment mode.
    pub list_tab_stop_position: Option<String>,
    /// `fo:text-indent` — hanging indent of the text block (ODF 1.2+).
    pub text_indent: Option<String>,
    /// `fo:margin-left` — left indent of the text block (ODF 1.2+).
    pub margin_left: Option<String>,

    /// Character formatting applied to the label.
    pub text_props: Option<OdfTextProps>,
}

/// The marker kind for an [`OdfListLevel`].
#[derive(Debug, Clone)]
pub(crate) enum OdfListLevelKind {
    /// Bullet list: a single Unicode character repeated at every item.
    ///
    /// ODF 1.3 §16.31 `text:list-level-style-bullet`.
    Bullet {
        /// `text:bullet-char` — the bullet character (e.g. `"•"`, `"–"`).
        char: String,
        /// `text:style-name` — character style applied to the bullet glyph.
        style_name: Option<String>,
    },

    /// Numbered list: a formatted counter per item.
    ///
    /// ODF 1.3 §16.33 `text:list-level-style-number`.
    Number {
        /// `style:num-format` — number style: `"1"`, `"a"`, `"A"`,
        /// `"i"`, `"I"`, etc.
        num_format: Option<String>,
        /// `style:num-prefix` — text prepended before the counter.
        num_prefix: Option<String>,
        /// `style:num-suffix` — text appended after the counter (e.g. `"."`).
        num_suffix: Option<String>,
        /// `text:start-value` — initial counter value for this level.
        start_value: Option<u32>,
        /// `text:display-levels` — how many ancestor-level counters to show.
        display_levels: u8,
        /// `text:style-name` — character style applied to the counter.
        style_name: Option<String>,
    },

    /// No visible label at this level.
    ///
    /// ODF 1.3 §16.34 `text:list-level-style-none`, or a number level
    /// whose `style:num-format` is empty.
    None,
}
