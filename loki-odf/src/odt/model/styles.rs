// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! ODF style model types.
//!
//! Covers `style:style`, `style:default-style`, paragraph properties
//! (`style:paragraph-properties`), and text properties
//! (`style:text-properties`) as defined in ODF 1.3 §14–§16.
//!
//! Property values are stored as raw attribute strings so that formatting
//! information is preserved verbatim regardless of the ODF version and can
//! be round-tripped without loss.

use super::document::{OdfMasterPage, OdfPageLayout};
use super::list_styles::OdfListStyle;

/// The complete set of styles extracted from a single ODF document.
///
/// Aggregates named styles from `styles.xml`, automatic styles from
/// `content.xml`, list styles, default styles, page layouts, and master
/// pages. ODF 1.3 §14.1, §16.5, §16.9.
#[derive(Debug, Clone, Default)]
pub(crate) struct OdfStylesheet {
    /// Named (user-facing) styles from `office:styles`. ODF 1.3 §14.1.
    pub named_styles: Vec<OdfStyle>,
    /// Automatic (paragraph/span-level) styles from
    /// `office:automatic-styles`. ODF 1.3 §14.1.
    pub auto_styles: Vec<OdfStyle>,
    /// List styles from `text:list-style`. ODF 1.3 §16.30.
    pub list_styles: Vec<OdfListStyle>,
    /// Default formatting per style family. ODF 1.3 §14.3.
    pub default_styles: Vec<OdfDefaultStyle>,
    /// Page layout definitions. ODF 1.3 §16.5 `style:page-layout`.
    pub page_layouts: Vec<OdfPageLayout>,
    /// Master page definitions. ODF 1.3 §16.9 `style:master-page`.
    pub master_pages: Vec<OdfMasterPage>,
}

impl OdfStylesheet {
    /// Append additional automatic styles (from `content.xml`) to this
    /// stylesheet's [`auto_styles`][Self::auto_styles] list.
    ///
    /// Called by [`crate::odt::import::OdtImporter::run`] after reading the
    /// `office:automatic-styles` section of `content.xml`.
    pub(crate) fn merge_auto(&mut self, styles: Vec<OdfStyle>) {
        self.auto_styles.extend(styles);
    }
}

/// A single named or automatic ODF style.
///
/// ODF 1.3 §14.1 `style:style`. Both named styles (from `office:styles`)
/// and automatic styles (from `office:automatic-styles`) use this type;
/// `is_automatic` distinguishes them.
#[derive(Debug, Clone)]
pub(crate) struct OdfStyle {
    /// `style:name` — the unique identifier used in element attributes.
    pub name: String,
    /// `style:display-name` — human-readable label shown in the UI.
    pub display_name: Option<String>,
    /// `style:family` — the element type this style applies to.
    pub family: OdfStyleFamily,
    /// `style:parent-style-name` — style inheritance chain.
    pub parent_name: Option<String>,
    /// `text:list-style-name` — associated list style, if any.
    pub list_style_name: Option<String>,
    /// Paragraph formatting properties, if present.
    pub para_props: Option<OdfParaProps>,
    /// Text (character) formatting properties, if present.
    pub text_props: Option<OdfTextProps>,
    /// `true` for styles from `office:automatic-styles`.
    pub is_automatic: bool,
}

/// The family of ODF elements a style applies to.
///
/// ODF 1.3 §19.480 `style:family`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OdfStyleFamily {
    /// `"paragraph"` — applies to `text:p` and `text:h`.
    Paragraph,
    /// `"text"` — applies to `text:span`.
    Text,
    /// `"table"` — applies to `table:table`.
    Table,
    /// `"table-row"` — applies to `table:table-row`.
    TableRow,
    /// `"table-cell"` — applies to `table:table-cell`.
    TableCell,
    /// `"graphic"` — applies to `draw:frame` and related elements.
    Graphic,
    /// Any other or unrecognised family value.
    Unknown,
}

/// Default style applied when no explicit style is set for a family.
///
/// ODF 1.3 §14.3 `style:default-style`.
#[derive(Debug, Clone)]
pub(crate) struct OdfDefaultStyle {
    /// The element family this default applies to.
    pub family: OdfStyleFamily,
    /// Default paragraph properties for this family.
    pub para_props: Option<OdfParaProps>,
    /// Default text properties for this family.
    pub text_props: Option<OdfTextProps>,
}

/// Paragraph formatting properties (`style:paragraph-properties`).
///
/// All length values are stored as raw ODF attribute strings (e.g. `"2.5cm"`)
/// so that formatting is preserved verbatim. ODF 1.3 §17.6.
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
}

/// Text (character) formatting properties (`style:text-properties`).
///
/// All values are raw ODF attribute strings. ODF 1.3 §20.2.
#[derive(Debug, Clone, Default)]
pub(crate) struct OdfTextProps {
    /// `style:font-name` — font face name from the font declarations.
    pub font_name: Option<String>,
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
}
