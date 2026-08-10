// SPDX-License-Identifier: Apache-2.0

//! The page style dialog's tab set, and the origin line that replaces
//! provenance for a non-inheriting family (design notes 11–14).

use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize};
use loki_doc_model::layout::paper_catalog;
use loki_doc_model::loki_primitives::units::MeasurementUnit;
use loki_i18n::fl;

/// The six tabs of the page style editor, in strip order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PageTab {
    /// Paper, orientation, and the resulting size.
    Page,
    /// Margins, mirroring, and the gutter.
    Margins,
    /// Column count, gap, and separator.
    Columns,
    /// Header geometry and content.
    Header,
    /// Footer geometry, content, and page numbering.
    Footer,
    /// Page borders and background.
    Borders,
}

impl PageTab {
    /// Every tab, in strip order.
    pub const ALL: [PageTab; 6] = [
        PageTab::Page,
        PageTab::Margins,
        PageTab::Columns,
        PageTab::Header,
        PageTab::Footer,
        PageTab::Borders,
    ];

    /// How many tabs keep an inline slot at Medium.
    ///
    /// Page, Margins and Columns: the geometry a user opens this dialog to
    /// change. Header, Footer and Borders are set once per document and then
    /// left alone, so they are the ones worth a click.
    pub const INLINE_AT_MEDIUM: usize = 3;

    /// The tab's position in the strip.
    #[must_use]
    pub fn index(self) -> usize {
        PageTab::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// The tab at `index`, saturating at the last tab.
    #[must_use]
    pub fn from_index(index: usize) -> Self {
        PageTab::ALL.get(index).copied().unwrap_or(PageTab::Borders)
    }

    /// The localized strip label.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            PageTab::Page => fl!("page-dialog-tab-page"),
            PageTab::Margins => fl!("page-dialog-tab-margins"),
            PageTab::Columns => fl!("page-dialog-tab-columns"),
            PageTab::Header => fl!("page-dialog-tab-header"),
            PageTab::Footer => fl!("page-dialog-tab-footer"),
            PageTab::Borders => fl!("page-dialog-tab-borders"),
        }
    }

    /// Every label, in strip order.
    #[must_use]
    pub fn labels() -> Vec<String> {
        PageTab::ALL.iter().map(|t| t.label()).collect()
    }
}

/// Where a page geometry's size comes from.
///
/// # Not a provenance chip
///
/// Page styles are a **non-inheriting** family — no `basedOn` parent in either
/// format (design note 11) — so there is no ancestor to name and no chain to
/// walk. The only upstream a page geometry has is the paper preset it matches,
/// and that is what this reports instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PaperOrigin {
    /// The size matches a catalogued paper.
    Preset {
        /// The paper's display name, e.g. `A4`.
        name: &'static str,
    },
    /// The size matches no catalogued paper.
    Custom,
}

impl PaperOrigin {
    /// The origin of `layout`'s page size.
    #[must_use]
    pub fn of(layout: &PageLayout) -> Self {
        match paper_catalog::paper_for(&layout.page_size) {
            Some(paper) => PaperOrigin::Preset {
                name: paper.display_name,
            },
            None => PaperOrigin::Custom,
        }
    }

    /// Whether the width and height boxes are read-only.
    ///
    /// Note 12: they are read-only while a named preset is selected, and typing
    /// into them switches Paper to **Custom** rather than silently
    /// desynchronising the label from the number beside it.
    #[must_use]
    pub fn is_preset(&self) -> bool {
        matches!(self, PaperOrigin::Preset { .. })
    }

    /// The localized origin line under the paper control.
    #[must_use]
    pub fn line(&self, layout: &PageLayout, unit: MeasurementUnit) -> String {
        let size = fl!(
            "page-dialog-size-value",
            width = unit.format_bare(layout.page_size.width),
            height = unit.format_bare(layout.page_size.height),
            unit = unit_label(unit)
        );
        match self {
            PaperOrigin::Preset { name } => {
                fl!(
                    "page-dialog-origin-preset",
                    name = (*name).to_string(),
                    size = size
                )
            }
            PaperOrigin::Custom => fl!("page-dialog-origin-custom", size = size),
        }
    }
}

/// The short unit suffix shown beside a measurement.
#[must_use]
pub(super) fn unit_label(unit: MeasurementUnit) -> String {
    match unit {
        MeasurementUnit::Millimeter => fl!("page-dialog-unit-mm"),
        MeasurementUnit::Centimeter => fl!("page-dialog-unit-cm"),
        MeasurementUnit::Inch => fl!("page-dialog-unit-in"),
        MeasurementUnit::Point => fl!("page-dialog-unit-pt"),
        MeasurementUnit::Pica => fl!("page-dialog-unit-pc"),
    }
}

/// Whether this layout mirrors its margins on facing pages.
///
/// Note 14: the margin labels are **Inner / Outer** while mirroring is on and
/// Left / Right when it is off — the label follows the model, not the screen
/// edge. The question is answered by [`PageUsage::mirrors_margins`], which the
/// model documents as its single derivation: `Left` and `Right` restrict *which*
/// pages a layout is used for rather than swapping margins within it, and
/// re-deciding that here is exactly the call site it warns about.
#[must_use]
pub(super) fn is_mirrored(layout: &PageLayout) -> bool {
    layout.page_usage.mirrors_margins()
}

/// The localized label for the start-edge margin.
#[must_use]
pub(super) fn start_margin_label(mirrored: bool) -> String {
    if mirrored {
        fl!("page-dialog-margin-inner")
    } else {
        fl!("page-dialog-margin-left")
    }
}

/// The localized label for the end-edge margin.
#[must_use]
pub(super) fn end_margin_label(mirrored: bool) -> String {
    if mirrored {
        fl!("page-dialog-margin-outer")
    } else {
        fl!("page-dialog-margin-right")
    }
}

/// Whether all four margins are equal.
///
/// Note 13: equality is tested **in points**, not in the display unit, so the
/// summary never flips as the unit changes — 25.4 mm and 1 in are the same
/// margin, and a comparison of their rounded display strings would disagree.
#[must_use]
pub(super) fn margins_are_equal(layout: &PageLayout) -> bool {
    let m = &layout.margins;
    let first = m.top.value();
    [m.bottom.value(), m.left.value(), m.right.value()]
        .iter()
        .all(|v| (v - first).abs() < f64::EPSILON)
}

/// The orientation a page size describes.
///
/// `PageLayout` stores the orientation *and* the size, so typing a landscape
/// width into the size boxes while the field still said Portrait left the two
/// disagreeing — and the field is the one the ODF and OOXML writers read.
#[must_use]
pub(super) fn orientation_of(size: &PageSize) -> PageOrientation {
    if size.width.value() > size.height.value() {
        PageOrientation::Landscape
    } else {
        PageOrientation::Portrait
    }
}

/// Which alternate band a header/footer toggle addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Variant {
    /// The first page of the style.
    First,
    /// Even (verso) pages.
    Even,
}

/// Whether `layout` carries a distinct band for `variant`.
///
/// This is the checkbox's `checked` state, and the checkbox is labelled
/// "Different first page" / "Different odd and even pages" — so `true` means a
/// separate band **exists**. It once meant the opposite, matching a "Same as…"
/// label that was never shipped, which inverted the control against its own
/// caption in both directions.
#[must_use]
pub(super) fn band_differs(layout: &PageLayout, header: bool, variant: Variant) -> bool {
    match (header, variant) {
        (true, Variant::First) => layout.header_first.is_some(),
        (true, Variant::Even) => layout.header_even.is_some(),
        (false, Variant::First) => layout.footer_first.is_some(),
        (false, Variant::Even) => layout.footer_even.is_some(),
    }
}

/// Adds or removes the distinct band for `variant`, so that afterwards
/// [`band_differs`] returns `differs`.
pub(super) fn set_band_variant(
    layout: &mut PageLayout,
    header: bool,
    variant: Variant,
    differs: bool,
) {
    let band = differs.then(|| {
        HeaderFooter::new(match variant {
            Variant::First => HeaderFooterKind::First,
            Variant::Even => HeaderFooterKind::Even,
        })
    });
    match (header, variant) {
        (true, Variant::First) => layout.header_first = band,
        (true, Variant::Even) => layout.header_even = band,
        (false, Variant::First) => layout.footer_first = band,
        (false, Variant::Even) => layout.footer_even = band,
    }
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
