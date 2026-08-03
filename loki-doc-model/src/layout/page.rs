// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Page layout types.
//!
//! TR 29166 §7.2.8 classifies section and page layout as "moderate to
//! difficult" translation. This module covers the 80% case.
//!
//! ODF: `style:page-layout` / `style:master-page`.
//! OOXML: `w:sectPr` (section properties) at the end of a section.

use crate::content::attr::ExtensionBag;
use crate::layout::header_footer::HeaderFooter;
use crate::style::list_style::NumberingScheme;
use crate::style::props::border::Border;
use loki_primitives::units::Points;

/// A decorative border drawn around each page of a section (`w:pgBorders`,
/// ECMA-376 §17.6.10). ODF: `style:page-layout` border properties.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageBorders {
    pub top: Option<Border>,
    pub left: Option<Border>,
    pub bottom: Option<Border>,
    pub right: Option<Border>,
    /// `true` when `@w:offsetFrom="text"` (each edge is inset from the text/margin
    /// area). `false` (the default `="page"`) insets from the physical page edge.
    /// Each edge's inset distance is carried in its [`Border::spacing`] (points).
    pub offset_from_text: bool,
}

impl PageBorders {
    /// `true` when no edge is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.top.is_none() && self.left.is_none() && self.bottom.is_none() && self.right.is_none()
    }
}

/// When the line-number counter restarts (OOXML `w:lnNumType @w:restart`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LineNumberRestart {
    /// Restart at the top of every page (`@w:restart="newPage"`, the default).
    #[default]
    NewPage,
    /// Restart at the start of every section (`@w:restart="newSection"`).
    NewSection,
    /// Never restart — number continuously across the document
    /// (`@w:restart="continuous"`).
    Continuous,
}

/// Line numbering displayed in the margin for a section (`w:lnNumType`,
/// ECMA-376 §17.6.8). ODF: `text:linenumbering-configuration`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineNumbering {
    /// Print a number every `count_by` lines (`@w:countBy`); e.g. `1` numbers
    /// every line, `5` every fifth. `0`/absent is treated as `1`.
    pub count_by: u32,
    /// The first line number (`@w:start`); defaults to `1`.
    pub start: i32,
    /// When the counter restarts (`@w:restart`).
    pub restart: LineNumberRestart,
    /// Distance from the numbers to the text, in points (`@w:distance`). `None`
    /// = automatic (the renderer picks a default gutter offset).
    pub distance: Option<Points>,
}

impl Default for LineNumbering {
    fn default() -> Self {
        Self {
            count_by: 1,
            start: 1,
            restart: LineNumberRestart::default(),
            distance: None,
        }
    }
}

/// Page orientation.
///
/// TR 29166 §7.2.8. ODF `style:print-orientation`; OOXML inferred from
/// page width/height relationship in `w:pgSz`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PageOrientation {
    /// Height > width (the default for most documents).
    #[default]
    Portrait,
    /// Width > height.
    Landscape,
}

/// The physical dimensions of a page.
///
/// TR 29166 §7.2.8. ODF: `fo:page-width` and `fo:page-height` on
/// `style:page-layout-properties`. OOXML: `w:pgSz` with `w:w` and `w:h`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageSize {
    /// The page width in points.
    pub width: Points,
    /// The page height in points.
    pub height: Points,
}

impl PageSize {
    /// ISO 216 A4 page size (595 × 842 pt).
    ///
    /// Defined by the catalogue entry that also *names* it, so the dimensions
    /// this returns and the dimensions a page is recognised by are one fact.
    #[must_use]
    pub fn a4() -> Self {
        crate::layout::paper_catalog::A4.portrait()
    }

    /// US Letter page size (612 × 792 pt) — likewise from the catalogue.
    #[must_use]
    pub fn letter() -> Self {
        crate::layout::paper_catalog::US_LETTER.portrait()
    }

    /// The catalogued paper this size is, or `None` for a user-defined size.
    /// Orientation-independent; see [`paper_for`].
    ///
    /// [`paper_for`]: crate::layout::paper_catalog::paper_for
    #[must_use]
    pub fn paper(&self) -> Option<&'static crate::layout::paper_catalog::Paper> {
        crate::layout::paper_catalog::paper_for(self)
    }
}

impl Default for PageSize {
    fn default() -> Self {
        Self::letter()
    }
}

/// Page margin distances from each edge.
///
/// TR 29166 §7.2.8. ODF: `fo:margin-*` and `fo:padding-*` on
/// `style:page-layout-properties`. OOXML: `w:pgMar`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageMargins {
    /// Distance from the top edge to the text area.
    pub top: Points,
    /// Distance from the bottom edge to the text area.
    pub bottom: Points,
    /// Distance from the left edge to the text area (start margin in LTR).
    pub left: Points,
    /// Distance from the right edge to the text area (end margin in LTR).
    pub right: Points,
    /// Space reserved for the header. ODF: `fo:margin-top` of the header;
    /// OOXML: `w:header`.
    pub header: Points,
    /// Space reserved for the footer. OOXML: `w:footer`.
    pub footer: Points,
    /// Gutter margin (extra space for binding). OOXML: `w:gutter`.
    pub gutter: Points,
}

impl Default for PageMargins {
    /// Standard 1-inch (72 pt) margins on all sides with 0.5-inch header/footer.
    fn default() -> Self {
        Self {
            top: Points::new(72.0),
            bottom: Points::new(72.0),
            left: Points::new(72.0),
            right: Points::new(72.0),
            header: Points::new(36.0),
            footer: Points::new(36.0),
            gutter: Points::new(0.0),
        }
    }
}

/// Multi-column section layout.
///
/// TR 29166 §7.2.8. ODF: `style:columns` inside `style:page-layout-properties`.
/// OOXML: `w:cols` inside `w:sectPr`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SectionColumns {
    /// The number of text columns.
    pub count: u8,
    /// The gap between columns in points (used between every pair of columns).
    pub gap: Points,
    /// Whether a separator line is drawn between columns.
    pub separator: bool,
    /// Explicit per-column widths in points (OOXML `w:cols w:equalWidth="0"` /
    /// ODF unequal `style:column` widths). **Empty** = equal columns (the width
    /// is derived from the content area, `count`, and `gap`). When present its
    /// length is `count`; the uniform [`gap`](Self::gap) still separates them.
    #[cfg_attr(feature = "serde", serde(default))]
    pub widths: Vec<Points>,
}

impl SectionColumns {
    /// Creates a two-column layout with the standard 18pt gap (equal widths).
    #[must_use]
    pub fn two_column() -> Self {
        Self {
            count: 2,
            gap: Points::new(18.0),
            separator: false,
            widths: Vec::new(),
        }
    }
}

pub use super::page_usage::PageUsage;

impl PageLayout {
    /// Sets the page size **and brings [`orientation`](Self::orientation) with
    /// it**.
    ///
    /// The two are one fact recorded twice: `w:orient` and `w:w`/`w:h` in OOXML,
    /// and in this model a `PageOrientation` beside a `PageSize`. Word keeps
    /// them agreeing — a landscape page has its width and height already
    /// swapped *and* `w:orient="landscape"` — and every consumer here assumes
    /// the same. They drifted wherever a caller assigned `page_size` on its own:
    /// a typed custom size or a seeded app default produced landscape
    /// dimensions under a `Portrait` flag, so the exporter wrote
    /// `w:orient="portrait"` for a page that is plainly landscape while the
    /// panel's Landscape button — which reads the dimensions — lit up.
    ///
    /// Assign through here rather than to the field, and the pair cannot part.
    pub fn set_page_size(&mut self, size: PageSize) {
        self.orientation = if size.width.value() > size.height.value() {
            PageOrientation::Landscape
        } else {
            PageOrientation::Portrait
        };
        self.page_size = size;
    }
}

/// The complete page layout for a section.
///
/// TR 29166 §7.2.8 (Section and page layout) and §6.2.3 (header/footer).
///
/// ODF: composed from `style:page-layout` + `style:master-page`.
/// OOXML: `w:sectPr` at the end of the section.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageLayout {
    /// The physical page size.
    pub page_size: PageSize,
    /// The page margins.
    pub margins: PageMargins,
    /// The page orientation.
    pub orientation: PageOrientation,
    /// Multi-column layout, if any. `None` = single column.
    pub columns: Option<SectionColumns>,
    /// Which pages of a spread this layout applies to, and whether its margins
    /// mirror (ODF `style:page-usage`; OOXML's document-wide
    /// `w:mirrorMargins` collapses into this on import).
    #[cfg_attr(feature = "serde", serde(default))]
    pub page_usage: PageUsage,
    /// The default (odd/right-page) header.
    pub header: Option<HeaderFooter>,
    /// The default (odd/right-page) footer.
    pub footer: Option<HeaderFooter>,
    /// First-page-only header.
    pub header_first: Option<HeaderFooter>,
    /// First-page-only footer.
    pub footer_first: Option<HeaderFooter>,
    /// Even-page header.
    pub header_even: Option<HeaderFooter>,
    /// Even-page footer.
    pub footer_even: Option<HeaderFooter>,
    /// Page-number display format for this section (OOXML `w:pgNumType @w:fmt`;
    /// ODF `style:num-format` on the page's master style). `None` = decimal.
    pub page_number_format: Option<NumberingScheme>,
    /// Page-number restart value for this section (OOXML `w:pgNumType @w:start`).
    /// `None` = continue numbering from the previous section.
    pub page_number_start: Option<u32>,
    /// Decorative border drawn around each page of the section (`w:pgBorders`).
    /// `None` = no page border.
    #[cfg_attr(feature = "serde", serde(default))]
    pub page_border: Option<PageBorders>,
    /// Margin line numbering for the section (`w:lnNumType`). `None` = off.
    #[cfg_attr(feature = "serde", serde(default))]
    pub line_numbering: Option<LineNumbering>,
    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
