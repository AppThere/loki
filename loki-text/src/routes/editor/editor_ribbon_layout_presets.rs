// SPDX-License-Identifier: Apache-2.0

//! The Layout tab's preset tables (split from `editor_ribbon_layout.rs` for
//! the 300-line ceiling): margin, page-size, and column quick presets, plus
//! the margin-match predicate that drives the active states.

use appthere_ui::{
    AT_COLUMNS_ONE, AT_COLUMNS_THREE, AT_COLUMNS_TWO, AT_MARGIN_NARROW, AT_MARGIN_NORMAL,
    AT_MARGIN_WIDE, AT_PAGE_A4, AT_PAGE_LETTER,
};
use loki_doc_model::layout::paper_catalog::{self, Paper};

/// A margin preset: `(aria-key, top, bottom, left, right, icon)` — points.
pub(super) const MARGIN_PRESETS: &[(&str, f64, f64, f64, f64, &str)] = &[
    (
        "ribbon-margin-normal-aria",
        72.0,
        72.0,
        72.0,
        72.0,
        AT_MARGIN_NORMAL,
    ),
    (
        "ribbon-margin-narrow-aria",
        36.0,
        36.0,
        36.0,
        36.0,
        AT_MARGIN_NARROW,
    ),
    (
        "ribbon-margin-wide-aria",
        72.0,
        72.0,
        144.0,
        144.0,
        AT_MARGIN_WIDE,
    ),
];

/// Whether the document's `current` margins match a preset `(top, bottom, left,
/// right)` within half a point — drives which preset button shows active.
pub(super) fn margin_matches(
    current: Option<(f64, f64, f64, f64)>,
    preset: (f64, f64, f64, f64),
) -> bool {
    let Some((t, b, l, r)) = current else {
        return false;
    };
    let close = |a: f64, x: f64| (a - x).abs() < 0.5;
    close(t, preset.0) && close(b, preset.1) && close(l, preset.2) && close(r, preset.3)
}

/// The ribbon's page-size quick presets: `(aria-key, paper, icon)`.
///
/// Two, deliberately — the ribbon is the two-click path for the common case,
/// and the **whole** catalogue lives in the page-style panel's size picker
/// (T6.2). The dimensions come from the catalogue rather than literals here,
/// which is what this row used to hold: a second copy of A4's and Letter's
/// measurements that could drift from the ones naming them.
pub(super) const PAGE_SIZE_PRESETS: &[(&str, &Paper, &str)] = &[
    ("ribbon-page-a4-aria", &paper_catalog::A4, AT_PAGE_A4),
    (
        "ribbon-page-letter-aria",
        &paper_catalog::US_LETTER,
        AT_PAGE_LETTER,
    ),
];

/// A column preset: `(aria-key, count, icon)`.
pub(super) const COLUMN_PRESETS: &[(&str, u8, &str)] = &[
    ("ribbon-columns-one-aria", 1, AT_COLUMNS_ONE),
    ("ribbon-columns-two-aria", 2, AT_COLUMNS_TWO),
    ("ribbon-columns-three-aria", 3, AT_COLUMNS_THREE),
];
