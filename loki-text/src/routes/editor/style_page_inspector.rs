// SPDX-License-Identifier: Apache-2.0

//! The read-only inspector model for **page styles** (Spec 05 M6 — the page
//! family, ADR-0012 Decision 2).
//!
//! Page styles are a **non-inheriting** family (no `basedOn` parent in either
//! format), so — like the list family — there is no provenance chain to resolve:
//! each geometry property is read directly from the style's [`PageLayout`]. This
//! module flattens a page style into one display row per property (size,
//! orientation, margins, columns) for the family panel, mirroring
//! `list_inspector_rows`'s role for its family.
//!
//! Pure + i18n-free: value-like text (`"A4"`, `"Portrait"`, `"72 pt"`) is baked
//! here the same way the list inspector bakes `"Bullet"`; the family panel
//! localises the surrounding field labels via each row's [`label_key`].

use loki_doc_model::layout::page::{PageLayout, PageMargins, PageOrientation, PageSize};
use loki_doc_model::loki_primitives::units::MeasurementUnit;

/// One display row of a page style: an i18n field-label key + its baked value.
pub struct PagePropRow {
    /// The Fluent key for the field label (e.g. `style-page-size`).
    pub label_key: &'static str,
    /// The property's value, formatted for display.
    pub value: String,
}

/// Builds the inspector rows for a page style's `layout`, in display order:
/// size, orientation, margins, columns.
#[must_use]
pub fn page_inspector_rows(layout: &PageLayout, unit: MeasurementUnit) -> Vec<PagePropRow> {
    vec![
        PagePropRow {
            label_key: "style-page-size",
            value: size_display(&layout.page_size, unit),
        },
        PagePropRow {
            label_key: "style-page-orientation",
            value: orientation_display(layout.orientation),
        },
        PagePropRow {
            label_key: "style-page-margins",
            value: margins_display(&layout.margins, unit),
        },
        PagePropRow {
            label_key: "style-page-columns",
            value: columns_display(layout),
        },
    ]
}

/// The catalogued paper's name (orientation-independent), else `W × H pt` for a
/// user-defined size.
///
/// The naming rule and the dimensions both come from
/// [`loki_doc_model::layout::paper_catalog`] — this used to carry its own copy
/// of both, and could name only the two sizes it had literals for.
fn size_display(size: &PageSize, unit: MeasurementUnit) -> String {
    match size.paper() {
        Some(paper) => paper.display_name.to_string(),
        None => format!(
            "{} × {} {}",
            unit.format_bare(size.width),
            unit.format_bare(size.height),
            unit.abbreviation()
        ),
    }
}

fn orientation_display(o: PageOrientation) -> String {
    match o {
        PageOrientation::Portrait => "Portrait",
        PageOrientation::Landscape => "Landscape",
    }
    .to_string()
}

/// A single `N <unit>` when all four edges are equal, else `T / B / L / R <unit>`.
///
/// The equality test stays in **points** rather than in the display unit: two
/// margins that differ by a hair are the same margin whichever unit is on
/// screen, and testing after rounding would make the "all four equal" answer
/// depend on the user's measurement setting.
fn margins_display(m: &PageMargins, unit: MeasurementUnit) -> String {
    let (t, b, l, r) = (
        m.top.value(),
        m.bottom.value(),
        m.left.value(),
        m.right.value(),
    );
    let eq = |a: f64, c: f64| (a - c).abs() < 0.5;
    let n = |v: f64| unit.format_bare(loki_doc_model::loki_primitives::units::Points::new(v));
    if eq(t, b) && eq(t, l) && eq(t, r) {
        format!("{} {}", n(t), unit.abbreviation())
    } else {
        format!(
            "{} / {} / {} / {} {}",
            n(t),
            n(b),
            n(l),
            n(r),
            unit.abbreviation()
        )
    }
}

fn columns_display(layout: &PageLayout) -> String {
    match &layout.columns {
        Some(c) if c.count > 1 => format!("{}", c.count),
        _ => "1".to_string(),
    }
}

#[cfg(test)]
#[path = "style_page_inspector_tests.rs"]
mod tests;
