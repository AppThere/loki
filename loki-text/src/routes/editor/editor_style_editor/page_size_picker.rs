// SPDX-License-Identifier: Apache-2.0

//! The page-**size** section of the page-style form (Spec 08 T6.2): one button
//! per catalogued paper, plus a custom width × height entry.
//!
//! Split from [`super::page_form`] so that file stays under the 300-line
//! ceiling, and because the catalogue grid and the custom field are the two
//! halves of one question — "what paper is this page?" — whose answers are
//! mutually exclusive (a custom size is exactly one the catalogue cannot name).
//!
//! # Why a wrapping grid rather than a dropdown
//!
//! The panel has no popover host of its own, and the catalogue is 28 entries —
//! small enough to show. T6.7's "catalogue with search" line wants a filtered
//! dropdown; that needs the panel hosted in `AtPopoverHost` first, so this is
//! the plain form of the control, not a substitute for it.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::layout::page::{PageLayout, PageSize};
use loki_doc_model::layout::paper_catalog::PAPERS;
use loki_doc_model::loki_primitives::units::MeasurementUnit;
use loki_i18n::fl;

use super::page_form::button_css;
use super::page_presets::PagePreset;

/// The smallest and largest page edge the custom-size field will accept, in
/// points (0.5 in to 200 in).
///
/// A page narrower than its margins lays out nothing, and an unbounded value
/// reaches the layout engine as a page count. Both ends are rejected rather
/// than clamped, so a typo does not silently become a different page.
const MIN_EDGE_PT: f64 = 36.0;
const MAX_EDGE_PT: f64 = 14400.0;

/// Parses a custom `width` × `height` entry into a [`PageSize`], reading bare
/// numbers in `unit` (T6.4) and honouring an explicit suffix such as `"8.5in"`.
/// `None` when either edge is unparseable or outside `MIN_EDGE_PT..=MAX_EDGE_PT`.
///
/// The range check is applied in **points**, after conversion: the limits are a
/// property of the page, not of the unit it was typed in, so 0.5 in and 12.7 mm
/// must be accepted or rejected alike.
///
/// Pure, so the acceptance rule is testable without a Dioxus scope.
#[must_use]
pub(super) fn parse_custom_size(
    width: &str,
    height: &str,
    unit: MeasurementUnit,
) -> Option<PageSize> {
    let ok = |s: &str| {
        unit.parse(s)
            .filter(|p| p.value().is_finite() && (MIN_EDGE_PT..=MAX_EDGE_PT).contains(&p.value()))
    };
    Some(PageSize {
        width: ok(width)?,
        height: ok(height)?,
    })
}

/// A width × height entry for a page size the catalogue does not name.
///
/// # Touch target
///
/// Two text inputs and a text button at the shared 24 px input height — the
/// same posture caveat as the rest of the form (see [`button_css`]).
#[component]
pub(super) fn CustomSizeField(
    current: PageSize,
    unit: MeasurementUnit,
    on_apply: EventHandler<PageSize>,
) -> Element {
    // Seeded from the page's current dimensions and keyed on them by the caller,
    // so selecting a different page style reseeds the fields rather than leaving
    // the previous style's numbers sitting in them.
    let mut w = use_signal(|| unit.format_bare(current.width));
    let mut h = use_signal(|| unit.format_bare(current.height));
    let parsed = parse_custom_size(&w.read(), &h.read(), unit);
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 4px; flex-wrap: wrap; margin-top: 4px;",
            span {
                style: format!(
                    "font-size: {fs}px; color: {fg}; min-width: 64px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-page-size-custom") }
            }
            input {
                r#type: "text",
                value: "{w}",
                oninput: move |evt| w.set(evt.value()),
                style: super::form_font::input_style("width: 52px"),
            }
            span {
                style: format!(
                    "font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                "×"
            }
            input {
                r#type: "text",
                value: "{h}",
                oninput: move |evt| h.set(evt.value()),
                style: super::form_font::input_style("width: 52px"),
            }
            span {
                style: format!(
                    "font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { unit.abbreviation() }
            }
            // Withheld rather than disabled while the entry is unusable: a
            // button that looks pressable and does nothing is the shape this
            // panel's `apply here` control already avoids.
            if let Some(size) = parsed {
                button {
                    style: button_css(false),
                    onclick: move |_| on_apply.call(size.clone()),
                    { fl!("style-page-size-apply") }
                }
            }
        }
    }
}

/// The catalogue grid + custom entry. `btn` builds one preset button (the
/// form's shared applier), `on_custom` commits a user-defined size.
///
/// `btn` is borrowed as a trait object rather than taken by value: the caller
/// keeps using the same closure for the margin and column rows after this one,
/// so a by-value parameter would move it out from under them.
pub(super) fn size_section(
    layout: &PageLayout,
    unit: MeasurementUnit,
    btn: &dyn Fn(String, PagePreset) -> Element,
    on_custom: impl FnMut(PageSize) + 'static,
) -> Element {
    let current = layout.page_size.clone();
    // Reseed the custom fields when the selected style's size changes **or the
    // unit does** — the seeded text is written in the unit, so a unit change
    // that did not reseed would leave millimetres sitting under an `in` label.
    let key = format!(
        "{:.0}x{:.0}@{}",
        current.width.value(),
        current.height.value(),
        unit.abbreviation()
    );
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: flex-start; gap: 6px; flex-wrap: wrap; margin-bottom: 4px;",
            span {
                style: format!(
                    "font-size: {fs}px; color: {fg}; min-width: 64px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-page-size") }
            }
            div {
                style: "display: flex; flex-direction: row; flex-wrap: wrap; gap: 4px; flex: 1;",
                for paper in PAPERS.iter() {
                    { btn(paper.display_name.to_string(), PagePreset::Size(paper)) }
                }
            }
        }
        // Wrapped so the keyed component is the first node of its own block:
        // Dioxus only honours `key` there, and a `key` it ignores would leave
        // the custom fields holding the previously selected style's numbers.
        div {
            CustomSizeField { key: "{key}", current, unit, on_apply: EventHandler::new(on_custom) }
        }
    }
}

#[cfg(test)]
#[path = "page_size_picker_tests.rs"]
mod tests;
