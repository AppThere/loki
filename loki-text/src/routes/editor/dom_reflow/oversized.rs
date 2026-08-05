// SPDX-License-Identifier: Apache-2.0

//! **T7.3's per-element horizontal scroll container** — the thing the reflow
//! view was moved to DOM for (ADR-0017 §3.1).
//!
//! # The rule, in one place
//!
//! *The document never scrolls horizontally.* The reading column says that once,
//! with `overflow-x: hidden` ([`super::document_view`]). An element too wide for
//! the column therefore has nowhere to overhang to — so it gets **its own**
//! scrollport here, and the overflow is that element's rather than the page's.
//!
//! On the canvas path this could not be built at all: there is no per-element
//! box in a wgpu tile to make scrollable, and a wide table was clipped
//! (`TODO(t7.3-element-scroller)` in `loki_renderer::doc_page_source`). Probe P1
//! measured that Blitz routes nested scroll containers correctly — consumes
//! within its bounds, bubbles the remainder — and that measurement is about
//! *this* code.
//!
//! # Fit and expand are different questions for different elements
//!
//! An **image** can be made to fit: scale it, aspect preserved. That is
//! `loki_layout`'s `fit_to_column` on the canvas path and `max-width: 100%` here
//! — the same rule, stated where the layout that applies it lives.
//!
//! A **table** cannot: its width comes from its column widths, and "fitting" it
//! means redistributing those, which is a different problem from scaling a
//! rectangle. So fit mode constrains what can be constrained and the scrollport
//! catches the rest. Either way the page does not move.
//!
//! Expanding is per element and per reader: a figure worth seeing at full size
//! is a decision no layout rule can take.

use dioxus::prelude::*;
use loki_i18n::fl;

use appthere_ui::tokens::{colors, spacing};

/// A block that may exceed the reading column, in its own scrollport.
///
/// # Touch target
///
/// The fit/expand toggle is **44 × 44 logical pixels** (WCAG 2.5.8): `min-width`
/// and `min-height` come from `appthere_ui::tokens::spacing::TOUCH_MIN` rather
/// than from the label, which is two words in English and one in several other
/// languages.
/// The scrollport's own declarations.
///
/// `overflow-x: auto` on *this* element is the whole of T7.3: whatever the child
/// does, the page does not move. `overflow-y: hidden` so a wide element cannot
/// also take the vertical gesture off the document — P1 measured that Blitz
/// bubbles the remainder correctly, and this keeps there being a remainder.
pub(super) const SCROLLPORT_CSS: &str = "overflow-x: auto; overflow-y: hidden;";

/// The child box's declarations, in each state.
///
/// **Fit** holds the child to the column, so anything that *can* shrink does and
/// no scrollbar appears; anything that cannot — a table at its column widths —
/// overflows into the scrollport, which is the better of the two bad outcomes
/// clipping used to pick.
///
/// **Expanded** gives the child its own width (`max-content`) and lets the
/// scrollport scroll. The constraint must be *dropped*, not raised: leaving
/// `max-width: 100%` on would make "expand" resolve against the same column and
/// do nothing at all.
#[must_use]
pub(super) fn inner_css(expanded: bool) -> &'static str {
    if expanded {
        "min-width: max-content;"
    } else {
        "max-width: 100%;"
    }
}

#[component]
pub(super) fn AtOversized(children: Element, fittable: bool) -> Element {
    // A component, not a function with an early return, because the state is
    // per element: two figures in one document expand independently, and only a
    // component owns a hook scope to keep that in (ADR-0013).
    let mut expanded = use_signal(|| false);

    // Fit/expand is a real choice only for an element that *can* be fitted. A
    // table at fixed column widths overflows the scrollport in either state, so
    // offering the toggle there would be a control that reports success and
    // changes nothing — the defect these tests were written against one level
    // up. The scrollport alone is the answer for it.
    let show_toggle = fittable;
    let label = if expanded() {
        fl!("editor-oversized-fit")
    } else {
        fl!("editor-oversized-expand")
    };
    let inner = inner_css(fittable && expanded());

    rsx! {
        div {
            style: format!(
                "position: relative; margin: {}px 0;",
                spacing::SPACE_2
            ),
            div {
                style: SCROLLPORT_CSS,
                div { style: "{inner}", {children} }
            }
            if show_toggle {
            button {
                style: format!(
                    "min-width: {min}px; min-height: {min}px; \
                     margin-top: {gap}px; padding: 0 {pad}px; \
                     background: {bg}; color: {fg}; border: none; \
                     border-radius: {r}px; font-size: 12px;",
                    min = spacing::TOUCH_MIN,
                    gap = spacing::SPACE_1,
                    pad = spacing::SPACE_2,
                    bg = colors::COLOR_SURFACE_1,
                    fg = colors::COLOR_TEXT_SECONDARY,
                    r = spacing::SPACE_1,
                ),
                onclick: move |_| expanded.toggle(),
                "{label}"
            }
            }
        }
    }
}

#[cfg(test)]
#[path = "oversized_tests.rs"]
mod tests;
