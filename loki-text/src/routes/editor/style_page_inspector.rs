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
pub fn page_inspector_rows(layout: &PageLayout) -> Vec<PagePropRow> {
    vec![
        PagePropRow {
            label_key: "style-page-size",
            value: size_display(&layout.page_size),
        },
        PagePropRow {
            label_key: "style-page-orientation",
            value: orientation_display(layout.orientation),
        },
        PagePropRow {
            label_key: "style-page-margins",
            value: margins_display(&layout.margins),
        },
        PagePropRow {
            label_key: "style-page-columns",
            value: columns_display(layout),
        },
    ]
}

/// A named paper size when the dimensions match A4 / US Letter (orientation-
/// independent, ±1 pt), else `W × H pt`.
fn size_display(size: &PageSize) -> String {
    let (w, h) = (size.width.value(), size.height.value());
    let (short, long) = (w.min(h), w.max(h));
    let matches = |a: &PageSize| {
        let (aw, ah) = (a.width.value(), a.height.value());
        let (as_, al) = (aw.min(ah), aw.max(ah));
        (short - as_).abs() < 1.0 && (long - al).abs() < 1.0
    };
    if matches(&PageSize::a4()) {
        "A4".to_string()
    } else if matches(&PageSize::letter()) {
        "US Letter".to_string()
    } else {
        format!("{w:.0} × {h:.0} pt")
    }
}

fn orientation_display(o: PageOrientation) -> String {
    match o {
        PageOrientation::Portrait => "Portrait",
        PageOrientation::Landscape => "Landscape",
    }
    .to_string()
}

/// A single `N pt` when all four edges are equal, else `T / B / L / R pt`.
fn margins_display(m: &PageMargins) -> String {
    let (t, b, l, r) = (
        m.top.value(),
        m.bottom.value(),
        m.left.value(),
        m.right.value(),
    );
    let eq = |a: f64, c: f64| (a - c).abs() < 0.5;
    if eq(t, b) && eq(t, l) && eq(t, r) {
        format!("{t:.0} pt")
    } else {
        format!("{t:.0} / {b:.0} / {l:.0} / {r:.0} pt")
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
