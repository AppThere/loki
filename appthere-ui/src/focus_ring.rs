// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The keyboard focus indicator, as a stylesheet every suite app injects
//! (Spec 08 T4.5 / r79).
//!
//! # Why this is a stylesheet and not a component prop
//!
//! WCAG 2.4.7 is a property of *every* focusable thing, and there is no list of
//! them: a `tabindex` on any div makes one. A per-component prop would therefore
//! be a rule that holds only where somebody remembered it, which is the failure
//! shape ADR-0013 is about. One `:focus` rule at the root covers every control
//! that exists and every control anyone adds later, without their cooperation.
//!
//! # What the sitting found
//!
//! Nothing in this workspace styled `:focus`, so focus moved and left no trace —
//! Tab walked the Home screen correctly and the screen never changed. That is not
//! a subtle defect, but it is invisible to every test that does not paint, which
//! is why it survived to be found by the first screen sitting (r79) rather than
//! by the suite.
//!
//! # `:focus`, not `:focus-visible`
//!
//! `:focus-visible` is what a browser-targeted design system would use — it is
//! how you avoid a ring on every mouse click. It **does not match in this
//! engine**: with the rule ordered last so it would win if it applied, the
//! `:focus` ring still painted (r79). So the choice here is not a preference,
//! and it comes with a known cost: a click leaves a ring on the clicked control.
//!
//! Marked as a COMPAT rather than worked around, because the workaround
//! available — tracking "was the last input a key" in the app and toggling a
//! class — would put an accessibility guarantee behind bookkeeping that can
//! drift, and a missing ring is worse than an extra one.

use crate::tokens::colors::COLOR_FOCUS_RING;

/// Width of the ring, in logical pixels.
///
/// 3px rather than the 2px a mouse-first design system would pick: this suite
/// runs on Android tablets with an attached keyboard, where the viewing distance
/// is greater than a desktop's and a hairline ring is the same as none.
const FOCUS_RING_WIDTH_PX: u32 = 3;

/// The focus-ring stylesheet, for injection into a `document::Style` at the app
/// root.
///
/// Every AppThere app must inject this. It is a function rather than a `const`
/// because the ring is composed from tokens, and a hand-written copy of the
/// same CSS in three apps is three places for the colour to drift.
///
/// ```no_run
/// # use dioxus::prelude::*;
/// # use appthere_ui::focus_ring_css;
/// # fn app() -> Element {
/// rsx! { document::Style { "{focus_ring_css()}" } }
/// # }
/// ```
#[must_use]
pub fn focus_ring_css() -> String {
    // COMPAT(dioxus-native): `outline` paints and `:focus` matches (both
    // confirmed by runtime probe, r79). `:focus-visible` and `outline-offset`
    // do not — the ring therefore sits on the border edge, which is why the
    // width is what carries the visibility rather than a gap.
    format!(":focus {{ outline: {FOCUS_RING_WIDTH_PX}px solid {COLOR_FOCUS_RING}; }}")
}

#[cfg(test)]
#[path = "focus_ring_tests.rs"]
mod tests;
