// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF style *property-value* model types (split from `styles.rs` for the
//! 300-line ceiling): paragraph properties, drop-cap and tab-stop specs, text
//! (character) properties, and cell properties. Values stay as raw ODF
//! attribute strings for lossless round-tripping. Re-exported from `styles.rs`
//! so existing `crate::odt::model::styles::Odf…` paths are unchanged.

/// Paragraph formatting properties (`style:paragraph-properties`); lengths
/// stay raw ODF attribute strings (e.g. `"2.5cm"`). ODF 1.3 §17.6.
#[derive(Debug, Clone, Default)]
pub(crate) struct OdfParaProps {
    /// `fo:margin-top` — space above the paragraph.
    pub margin_top: Option<String>,
    /// `fo:margin-bottom` — space below the paragraph.
    pub margin_bottom: Option<String>,
    /// `fo:margin-left` — left indent.
    pub margin_left: Option<String>,
    /// `fo:margin-right` — right indent.
    pub margin_right: Option<String>,
    /// `fo:text-indent` — first-line indent (may be negative for hanging).
    pub text_indent: Option<String>,
    /// `fo:line-height` — line spacing (absolute or percentage).
    pub line_height: Option<String>,
    /// `style:line-height-at-least` — minimum line height.
    pub line_height_at_least: Option<String>,
    /// `fo:text-align` — horizontal alignment: `"start"`, `"end"`,
    /// `"center"`, `"justify"`.
    pub text_align: Option<String>,
    /// `fo:keep-together` — prevent paragraph from splitting across pages.
    pub keep_together: Option<String>,
    /// `fo:keep-with-next` — keep this paragraph on the same page as the
    /// following one.
    pub keep_with_next: Option<String>,
    /// `fo:widows` — minimum lines at top of page.
    pub widows: Option<u8>,
    /// `fo:orphans` — minimum lines at bottom of page.
    pub orphans: Option<u8>,
    /// `fo:break-before` — page or column break before the paragraph.
    pub break_before: Option<String>,
    /// `fo:break-after` — page or column break after the paragraph.
    pub break_after: Option<String>,
    /// `fo:border` — shorthand border (all sides).
    pub border: Option<String>,
    /// `fo:border-top` — top border.
    pub border_top: Option<String>,
    /// `fo:border-bottom` — bottom border.
    pub border_bottom: Option<String>,
    /// `fo:border-left` — left border.
    pub border_left: Option<String>,
    /// `fo:border-right` — right border.
    pub border_right: Option<String>,
    /// `fo:padding` — shorthand padding (all sides).
    pub padding: Option<String>,
    /// `fo:background-color` — paragraph background (`"#RRGGBB"` or
    /// `"transparent"`).
    pub background_color: Option<String>,
    /// Tab stops defined within this style. ODF 1.3 §17.8.
    pub tab_stops: Vec<OdfTabStop>,
    /// `style:writing-mode` — text direction, e.g. `"lr-tb"`, `"rl-tb"`.
    pub writing_mode: Option<String>,
    /// `style:drop-cap` child element, if present. ODF 1.3 §20.342.
    pub drop_cap: Option<OdfDropCap>,
}

/// `style:drop-cap` element (ODF 1.3 §20.342). Raw attribute strings.
#[derive(Debug, Clone, Default)]
pub(crate) struct OdfDropCap {
    /// `style:lines` — number of lines the cap spans.
    pub lines: Option<String>,
    /// `style:length` — `"word"` or an integer character count.
    pub length: Option<String>,
    /// `style:distance` — gap between cap and body text (length).
    pub distance: Option<String>,
}

/// A single tab stop within a paragraph style.
///
/// ODF 1.3 §17.8 `style:tab-stop`.
#[derive(Debug, Clone)]
pub(crate) struct OdfTabStop {
    /// `style:position` — distance from the left margin (e.g. `"2.5cm"`).
    pub position: String,
    /// `style:type` — alignment: `"left"`, `"right"`, `"center"`, `"char"`.
    pub tab_type: Option<String>,
    /// `style:leader-style` — leader character style (ODF 1.3 §17.8).
    pub leader_style: Option<String>,
}

/// Text (character) formatting properties (`style:text-properties`).
///
/// All values are raw ODF attribute strings. ODF 1.3 §20.2.
#[derive(Debug, Clone, Default)]
pub(crate) struct OdfTextProps {
    /// `style:font-name` — font face name from the font declarations.
    pub font_name: Option<String>,
    /// `fo:font-family` — raw font family name (fallback when `font_name` absent).
    pub font_family: Option<String>,
    /// `fo:font-size` — font size (e.g. `"12pt"`).
    pub font_size: Option<String>,
    /// `fo:font-weight` — `"bold"`, `"normal"`, or numeric weight.
    pub font_weight: Option<String>,
    /// `fo:font-style` — `"italic"`, `"normal"`, `"oblique"`.
    pub font_style: Option<String>,
    /// `style:text-underline-style` — underline style.
    pub text_underline_style: Option<String>,
    /// `style:text-underline-type` — `"single"`, `"double"`, etc.
    pub text_underline_type: Option<String>,
    /// `style:text-line-through-style` — strikethrough style.
    pub text_line_through_style: Option<String>,
    /// `fo:font-variant` — `"small-caps"` or `"normal"`.
    pub font_variant: Option<String>,
    /// `fo:text-transform` — `"uppercase"`, `"lowercase"`, `"capitalize"`.
    pub text_transform: Option<String>,
    /// `fo:color` — foreground colour (`"#RRGGBB"`).
    pub color: Option<String>,
    /// `fo:background-color` — highlight / background colour.
    pub background_color: Option<String>,
    /// `fo:text-shadow` — drop shadow specification.
    pub text_shadow: Option<String>,
    /// `fo:language` — BCP 47 primary language subtag.
    pub language: Option<String>,
    /// `fo:country` — BCP 47 region subtag.
    pub country: Option<String>,
    /// `style:text-position` — super/subscript: `"super"`, `"sub"`, or
    /// percentage offset string.
    pub text_position: Option<String>,
    /// `fo:letter-spacing` — character spacing (e.g. `"0.5pt"`).
    pub letter_spacing: Option<String>,
    /// `style:font-size-complex` — font size for complex scripts.
    pub font_size_complex: Option<String>,
    /// `style:font-name-complex` — font name for complex scripts.
    pub font_name_complex: Option<String>,
    /// `style:font-name-asian` — font name for East Asian text.
    pub font_name_asian: Option<String>,
    /// `style:text-outline` — hollow/outline text effect.
    pub text_outline: Option<bool>,
    /// `fo:word-spacing` — additional space between words (e.g. `"0.2cm"`).
    pub word_spacing: Option<String>,
    /// `style:letter-kerning` — enable font kerning (`"true"` / `"false"`).
    pub letter_kerning: Option<bool>,
    /// `style:text-scale` — horizontal scale percentage (e.g. `"150%"`).
    pub text_scale: Option<String>,
    /// `fo:language` complex-script language subtag.
    pub language_complex: Option<String>,
    /// `fo:country` complex-script region subtag.
    pub country_complex: Option<String>,
    /// `fo:language` East Asian language subtag.
    pub language_asian: Option<String>,
    /// `fo:country` East Asian region subtag.
    pub country_asian: Option<String>,
}

/// Formatting properties from `style:table-cell-properties`.
///
/// All length and colour values are stored as raw ODF attribute strings so
/// they can be parsed lazily during mapping. ODF 1.3 §17.18.
// COMPAT(odf): style:table-cell-properties may appear as a self-closing
// element (Empty event) or with child elements (Start/End). Most producers
// use the self-closing form with all properties as attributes.
#[derive(Debug, Clone, Default)]
pub(crate) struct OdfCellProps {
    /// `fo:padding-top` or shorthand `fo:padding`.
    pub padding_top: Option<String>,
    /// `fo:padding-bottom` or shorthand `fo:padding`.
    pub padding_bottom: Option<String>,
    /// `fo:padding-left` or shorthand `fo:padding`.
    pub padding_left: Option<String>,
    /// `fo:padding-right` or shorthand `fo:padding`.
    pub padding_right: Option<String>,
    /// `style:vertical-align` — `"top"`, `"middle"`, `"bottom"`, `"automatic"`.
    pub vertical_align: Option<String>,
    /// `style:writing-mode` — `"lr-tb"`, `"tb-rl"`, `"tb-lr"`, `"bt-lr"`, etc.
    pub writing_mode: Option<String>,
    /// `fo:background-color` — hex colour e.g. `"#FFFF00"` or `"transparent"`.
    pub background_color: Option<String>,
    /// `fo:border-top` or shorthand `fo:border`.
    pub border_top: Option<String>,
    /// `fo:border-bottom` or shorthand `fo:border`.
    pub border_bottom: Option<String>,
    /// `fo:border-left` or shorthand `fo:border`.
    pub border_left: Option<String>,
    /// `fo:border-right` or shorthand `fo:border`.
    pub border_right: Option<String>,
}
