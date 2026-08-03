// SPDX-License-Identifier: Apache-2.0

//! The page form's **unit-aware margin entry** (Spec 08 T6.7, unit handling from
//! T6.4): four fields — top, bottom, left, right — read in the active
//! measurement unit.
//!
//! # Why the presets were not enough
//!
//! Normal / Narrow / Wide are three points in a continuous space. Every margin
//! the three do not name — a binding gutter, a publisher's spec, anything
//! metric — was unreachable from the panel, while the model, both importers and
//! both exporters carried arbitrary values. That is the same shape as the
//! 1–3 column limit T6.7 called out: a UI cap on a model that never had one.
//!
//! The presets stay as shortcuts, which is what the spec line asks for.
//!
//! # What this does not touch
//!
//! `header`, `footer` and `gutter` distances. They are part of
//! [`PageMargins`] but not of "the margins" as the presets use the word — the
//! preset arm sets the same four edges and leaves those three alone, and an
//! entry field that silently reset a gutter would be worse than an absent one.
//! `TODO(page-gutter-field)`: the spec line names gutter; it wants its own
//! labelled control rather than a fifth box in this row.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::layout::page::{PageLayout, PageMargins};
use loki_doc_model::loki_primitives::units::{MeasurementUnit, Points};
use loki_i18n::fl;

use super::page_form::button_css;

/// The smallest and largest margin the fields will accept, in points
/// (0 to 20 in).
///
/// Zero is allowed — a full-bleed page is a real design, and the layout engine
/// handles it — but a negative margin is not a page, and an unbounded one puts
/// the content area at zero width, which lays out nothing. Both ends are
/// rejected rather than clamped, so a typo does not silently become a different
/// page.
const MIN_MARGIN_PT: f64 = 0.0;
const MAX_MARGIN_PT: f64 = 1440.0;

/// The four edges as `(label, current value)`, in the order they are shown.
///
/// One list, walked by both the field row and the parser, so a field can never
/// be read into the edge next to it — the failure a fourfold copy-paste invites.
fn edges(m: &PageMargins) -> [(&'static str, Points); 4] {
    [
        ("style-page-margin-top", m.top),
        ("style-page-margin-bottom", m.bottom),
        ("style-page-margin-left", m.left),
        ("style-page-margin-right", m.right),
    ]
}

/// Parses four margin entries into a [`PageMargins`], reading bare numbers in
/// `unit` and honouring an explicit suffix such as `"0.75in"`.
///
/// `base` supplies everything this does not edit — `header`, `footer`, `gutter`
/// — so the result is the page's margins with four edges replaced, not a fresh
/// [`PageMargins`] that quietly zeroed the rest.
///
/// `None` when any edge is unparseable or outside `MIN_MARGIN_PT..=MAX_MARGIN_PT`.
/// The range check is applied in **points**, after conversion: the limits are a
/// property of the page, not of the unit it was typed in.
///
/// Pure, so the acceptance rule is testable without a Dioxus scope.
#[must_use]
pub(super) fn parse_margins(
    values: &[String; 4],
    unit: MeasurementUnit,
    base: &PageMargins,
) -> Option<PageMargins> {
    let ok = |s: &str| {
        unit.parse(s).filter(|p| {
            p.value().is_finite() && (MIN_MARGIN_PT..=MAX_MARGIN_PT).contains(&p.value())
        })
    };
    Some(PageMargins {
        top: ok(&values[0])?,
        bottom: ok(&values[1])?,
        left: ok(&values[2])?,
        right: ok(&values[3])?,
        ..base.clone()
    })
}

/// The four margin entries plus a Set button.
///
/// Seeded from the page's current margins and keyed on them by the caller, so
/// selecting a different page style — or changing the unit — reseeds the fields
/// rather than leaving the previous numbers under a new label.
///
/// # Touch target
///
/// Four text inputs and a text button at the shared 24 px input height — the
/// same posture caveat as the rest of the form (see [`button_css`]).
#[component]
pub(super) fn MarginFields(
    current: PageMargins,
    unit: MeasurementUnit,
    on_apply: EventHandler<PageMargins>,
) -> Element {
    let seeded = edges(&current).map(|(_, v)| unit.format_bare(v));
    let mut values = use_signal(|| seeded);
    let parsed = parse_margins(&values.read(), unit, &current);
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 4px; flex-wrap: wrap; margin-bottom: 4px;",
            span {
                style: format!(
                    "font-size: {fs}px; color: {fg}; min-width: 64px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-page-margins-custom") }
            }
            for (i, (label, _)) in edges(&current).into_iter().enumerate() {
                div {
                    key: "{label}",
                    style: "display: flex; flex-direction: row; align-items: center; gap: 2px;",
                    span {
                        style: format!(
                            "font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        { fl!(label) }
                    }
                    input {
                        r#type: "text",
                        value: "{values.read()[i]}",
                        oninput: move |evt| values.write()[i] = evt.value(),
                        style: super::form_font::input_style("width: 44px"),
                    }
                }
            }
            span {
                style: format!(
                    "font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { unit.abbreviation() }
            }
            // Withheld rather than disabled while an entry is unusable, matching
            // the custom-size field: a button that looks pressable and does
            // nothing is the shape this panel already avoids.
            if let Some(margins) = parsed {
                button {
                    style: button_css(false),
                    onclick: move |_| on_apply.call(margins.clone()),
                    { fl!("style-page-margins-apply") }
                }
            }
        }
    }
}

/// The key that reseeds [`MarginFields`] — the four edges plus the unit.
///
/// The unit belongs in it because the seeded text is written *in* the unit: a
/// unit change that did not reseed would leave millimetres sitting under an
/// `in` label. Same reasoning as the size field's key, and the same failure if
/// it is dropped.
#[must_use]
pub(super) fn margin_field_key(layout: &PageLayout, unit: MeasurementUnit) -> String {
    let m = &layout.margins;
    format!(
        "{:.1}/{:.1}/{:.1}/{:.1}@{}",
        m.top.value(),
        m.bottom.value(),
        m.left.value(),
        m.right.value(),
        unit.abbreviation()
    )
}

#[cfg(test)]
#[path = "page_margin_fields_tests.rs"]
mod tests;
