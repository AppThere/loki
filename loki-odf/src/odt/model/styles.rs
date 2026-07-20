// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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

#[path = "styles_props.rs"]
mod props;
pub(crate) use props::{OdfCellProps, OdfDropCap, OdfParaProps, OdfTabStop, OdfTextProps};

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
    /// `style:column-width` from `style:table-column-properties`, if present.
    /// Raw ODF length string (e.g. `"4cm"`). Only set for
    /// `style:family="table-column"` styles.
    // COMPAT(odf): column width from style:table-column-properties
    pub col_width: Option<String>,
    /// Properties for `style:family="table-cell"` styles.
    pub cell_props: Option<OdfCellProps>,
    /// `style:wrap` / `style:run-through` from `style:graphic-properties`, if
    /// present. Only set for `style:family="graphic"` styles applied to frames.
    pub graphic_wrap: Option<OdfGraphicWrap>,
    /// `true` for styles from `office:automatic-styles`.
    pub is_automatic: bool,
    /// Properties for `style:family="table"` styles (`style:table-properties`).
    pub table_props: Option<crate::odt::model::tables::OdfTableProps>,
    /// `style:master-page-name` — for paragraph styles, the master page this
    /// style transitions to when applied (its layout applies from that
    /// paragraph onward — ODF 1.3 §16.9). `None`/empty means no transition.
    pub master_page_name: Option<String>,
}

/// `style:graphic-properties` wrap attributes (ODF 1.3 §20.x). Raw strings.
#[derive(Debug, Clone, Default)]
pub(crate) struct OdfGraphicWrap {
    /// `style:wrap` — `"none"`, `"parallel"`, `"run-through"`, `"left"`,
    /// `"right"`, `"dynamic"`, `"biggest"`.
    pub wrap: Option<String>,
    /// `style:run-through` — `"foreground"` or `"background"` (behind text).
    pub run_through: Option<String>,
    /// `draw:fill-color` — frame solid-fill colour (`"#RRGGBB"`), when
    /// `draw:fill="solid"`. Used to recover a floating text box's fill.
    pub fill_color: Option<String>,
    /// `svg:stroke-color` — frame border colour (`"#RRGGBB"`), when
    /// `draw:stroke="solid"`. Used to recover a floating text box's border.
    pub stroke_color: Option<String>,
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

/// Default style applied when no explicit style is set for a family
/// (ODF 1.3 §14.3 `style:default-style`).
#[derive(Debug, Clone)]
pub(crate) struct OdfDefaultStyle {
    /// The element family this default applies to.
    pub family: OdfStyleFamily,
    /// Default paragraph properties for this family.
    pub para_props: Option<OdfParaProps>,
    /// Default text properties for this family.
    pub text_props: Option<OdfTextProps>,
    /// Default table properties (4a.3).
    pub table_props: Option<crate::odt::model::tables::OdfTableProps>,
}
