// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Page layout types.
//!
//! TR 29166 §7.2.8 classifies section and page layout as "moderate to
//! difficult" translation. This module covers the 80% case.
//!
//! ODF: `style:page-layout` / `style:master-page`.
//! OOXML: `w:sectPr` (section properties) at the end of a section.

use loki_primitives::units::Points;
use crate::content::attr::ExtensionBag;
use crate::layout::header_footer::HeaderFooter;

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
    #[must_use]
    pub fn a4() -> Self {
        Self {
            width: Points::new(595.28),
            height: Points::new(841.89),
        }
    }

    /// US Letter page size (612 × 792 pt).
    #[must_use]
    pub fn letter() -> Self {
        Self {
            width: Points::new(612.0),
            height: Points::new(792.0),
        }
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
    /// The gap between columns in points.
    pub gap: Points,
    /// Whether a separator line is drawn between columns.
    pub separator: bool,
}

impl SectionColumns {
    /// Creates a two-column layout with the standard 18pt gap.
    #[must_use]
    pub fn two_column() -> Self {
        Self {
            count: 2,
            gap: Points::new(18.0),
            separator: false,
        }
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
    /// Format-specific extension data.
    pub extensions: ExtensionBag,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_dimensions() {
        let size = PageSize::a4();
        // A4 is approximately 595 × 842 pt
        assert!((size.width.value() - 595.28).abs() < 0.1);
        assert!((size.height.value() - 841.89).abs() < 0.1);
    }

    #[test]
    fn default_page_layout_portrait() {
        let layout = PageLayout::default();
        assert_eq!(layout.orientation, PageOrientation::Portrait);
        assert!(layout.header.is_none());
    }
}
