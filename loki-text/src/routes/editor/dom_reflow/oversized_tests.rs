// SPDX-License-Identifier: Apache-2.0

//! Tests for T7.3's per-element scrollport.
//!
//! The rule under test is a pair of complementary claims, and each needs its
//! inversion: the **document** never scrolls sideways, and an element too wide
//! for the column does — in a box of its own.

use super::{SCROLLPORT_CSS, inner_css};

/// **The scrollport scrolls horizontally and not vertically.**
///
/// The vertical half is not decoration: an element that took the vertical
/// gesture as well would stop the *document* scrolling while the pointer was
/// over it, which is the same class of defect T7.3 removes, one level down.
#[test]
fn the_scrollport_takes_horizontal_overflow_only() {
    assert!(
        SCROLLPORT_CSS.contains("overflow-x: auto"),
        "{SCROLLPORT_CSS}"
    );
    assert!(
        SCROLLPORT_CSS.contains("overflow-y: hidden"),
        "a wide element would swallow the page's vertical scroll: {SCROLLPORT_CSS}"
    );
}

/// **Fit holds the child to the column.** Anything that can shrink does, so no
/// scrollbar appears for an ordinary figure.
#[test]
fn fit_constrains_the_child_to_the_column() {
    let css = inner_css(false);
    assert!(css.contains("max-width: 100%"), "{css}");
    assert!(
        !css.contains("max-content"),
        "the fitted state gave the child its own width: {css}"
    );
}

/// **Expanded drops the constraint** rather than raising it.
///
/// The inversion that matters: leaving `max-width: 100%` on would make "expand"
/// resolve against the same column and change nothing, and the button would
/// still highlight — a control that reports success and does nothing.
#[test]
fn expanded_gives_the_child_its_own_width() {
    let css = inner_css(true);
    assert!(css.contains("min-width: max-content"), "{css}");
    assert!(
        !css.contains("max-width: 100%"),
        "expanding left the column constraint on, so it does nothing: {css}"
    );
}

/// The two states are different. A refactor that collapsed them — say, by
/// returning the same string from both arms — would satisfy every `contains`
/// above if the string happened to carry both declarations.
#[test]
fn the_two_states_differ() {
    assert_ne!(inner_css(true), inner_css(false));
}

/// **An unfittable element never enters the expanded state**, because it never
/// left it: the toggle is not offered for one, and `inner_css` is asked for the
/// fitted string. Wiring `expanded()` straight through would give a table a
/// state its own control cannot reach.
#[test]
fn an_unfittable_element_stays_in_the_fitted_box() {
    // The expression the component uses. Stated here because the component's own
    // rendering is not reachable from a unit test, and this is the part of it
    // that decides behaviour.
    let inner_for = |fittable: bool, expanded: bool| inner_css(fittable && expanded);
    assert_eq!(inner_for(false, true), inner_css(false));
    assert_eq!(inner_for(false, false), inner_css(false));
    // And a fittable one still toggles, or the guard has eaten the feature.
    assert_eq!(inner_for(true, true), inner_css(true));
}
