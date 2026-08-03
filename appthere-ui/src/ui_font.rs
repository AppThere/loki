// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere contributors

//! The UI typeface, declared once at the document root.
//!
//! # Why this exists: the family was opt-in, so forgetting it was the default
//!
//! Every component in this crate set `font-family` on its own root — about
//! twenty-five copies of one fact — and nothing declared it for the document. So
//! an element whose subtree root did not set it fell through to the CSS initial
//! value, which is **serif**. Not a fallback; the wrong face entirely.
//!
//! That is invisible to review in the ordinary case, because most new elements
//! are nested inside something that already declared a family and inherit it.
//! It becomes visible exactly where a component is mounted **outside** the styled
//! subtree — which is what [`crate::AtPopoverHost`] does by construction: it
//! renders at the app root, deliberately outside every `overflow` ancestor, so a
//! menu's content inherits from `body` and nothing else. The zoom menu's rows
//! shipped in serif for that reason (Spec 08 r94, seen in
//! `scripts/sitting/run.sh zoom`).
//!
//! Fixing it at the twenty-sixth component would have left the twenty-seventh to
//! find. Declared here, an element that says nothing about its font gets the
//! right one, and a component only writes `font-family` when it wants something
//! *different* — which is what a declaration should mean.
//!
//! A function rather than a `const` for [`crate::focus_ring_css`]'s reason: the
//! sheet is composed from a token, and a hand-written copy in three apps is
//! three places for it to drift.

use crate::tokens::typography::FONT_FAMILY_UI;

/// The root typography sheet, for injection into a `document::Style` at the app
/// root. **Every AppThere app must inject this.**
///
/// ```no_run
/// # use dioxus::prelude::*;
/// # use appthere_ui::ui_font_css;
/// # fn app() -> Element {
/// rsx! { document::Style { "{ui_font_css()}" } }
/// # }
/// ```
///
/// # The form-control rule is not decoration
///
/// `button`, `input`, `select` and `textarea` do not inherit typography in a
/// browser — the HTML UA stylesheet gives them a `font` of their own, and every
/// CSS reset in existence undoes it. Blitz's UA sheet
/// (`patches/blitz-dom/assets/default.css`) happens **not** to carry that rule
/// today, so these controls do inherit here and the second selector is currently
/// redundant. It is written anyway: the day the vendored sheet gains the
/// standard rule, every menu row, every ribbon `select` and every dialog field
/// in the suite would silently change face, and the failure would present as
/// "the fonts broke in a Blitz bump" rather than as a missing reset. One line
/// now against a whole-suite regression later.
#[must_use]
pub fn ui_font_css() -> String {
    format!(
        "html, body {{ font-family: {FONT_FAMILY_UI}; }}\n\
         button, input, select, textarea {{ font-family: inherit; }}"
    )
}

#[cfg(test)]
#[path = "ui_font_tests.rs"]
mod tests;
