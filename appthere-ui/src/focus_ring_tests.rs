// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What a unit test can hold about a focus ring, and what it cannot.
//!
//! It cannot say the ring is *visible* — that took a screen sitting, and the
//! sitting is what found there was no ring at all. What it can hold is that the
//! rule keeps naming a focus selector and a colour that is not the accent, which
//! are the two ways this silently stops working: someone deletes the selector, or
//! someone "tidies" the ring onto the accent token and the primary button loses
//! its indicator against its own fill.

use super::focus_ring_css;
use crate::tokens::colors::{COLOR_ACCENT_PRIMARY, COLOR_FOCUS_RING};

/// The rule targets focus and draws an outline. Written against the string
/// because the string is what the engine parses.
#[test]
fn the_rule_styles_focus_with_an_outline() {
    let css = focus_ring_css();
    assert!(css.contains(":focus"), "{css}");
    assert!(css.contains("outline:"), "{css}");
    assert!(css.contains(COLOR_FOCUS_RING), "{css}");
}

/// **The ring is not the accent, and that is the point of the token.** A ring in
/// the accent colour disappears on an accent-filled button — the primary action
/// on the screen, and usually the first thing a keyboard user reaches.
#[test]
fn the_ring_is_not_the_accent_colour() {
    assert_ne!(
        COLOR_FOCUS_RING, COLOR_ACCENT_PRIMARY,
        "a focus ring the same colour as the primary button's fill is invisible \
         exactly where it matters most",
    );
    assert!(!focus_ring_css().contains(COLOR_ACCENT_PRIMARY));
}

/// **The polarity (L08-045).** The assertions above pass for a rule that styles
/// everything, not just focus — so pin that the selector is a bare `:focus` and
/// not something broader that happens to contain the substring.
#[test]
fn the_rule_applies_to_focus_only() {
    let css = focus_ring_css();
    assert!(
        css.starts_with(":focus {"),
        "the rule must be scoped to :focus, got {css}",
    );
    assert!(
        !css.contains('*'),
        "a universal selector would outline every element, got {css}",
    );
}

/// The width is a real length. A `0px` ring parses, applies, and shows nothing —
/// the one way this rule can be present and still fail its purpose.
#[test]
fn the_ring_has_a_width_that_can_be_seen() {
    let css = focus_ring_css();
    let width: u32 = css
        .split("outline: ")
        .nth(1)
        .and_then(|s| s.split("px").next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert!(
        width >= 2,
        "a hairline ring is not an indicator, got {width}px"
    );
}
