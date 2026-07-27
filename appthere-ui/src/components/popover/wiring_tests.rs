// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The two wiring decisions, asserted as the failures a user would hit.

use super::super::geometry::Rect;
use super::{is_outside_dismiss, open_response, OpenResponse, PopoverId};

const POPOVER: Rect = Rect {
    x: 100.0,
    y: 130.0,
    width: 300.0,
    height: 200.0,
};
const ANCHOR: Rect = Rect {
    x: 100.0,
    y: 100.0,
    width: 60.0,
    height: 24.0,
};

/// **The defect every popover ships once:** the trigger counts as outside, the
/// popover dismisses, the trigger's handler reopens it, and the control appears
/// not to toggle.
#[test]
fn clicking_the_anchor_is_not_an_outside_click() {
    assert!(
        !is_outside_dismiss((120.0, 110.0), POPOVER, ANCHOR),
        "a click on the trigger must be the trigger's business, or dismissal \
         and re-open race and the button stops toggling",
    );
}

/// A click inside the popover is obviously not a dismissal — kept so the anchor
/// exclusion is not mistaken for the whole rule.
#[test]
fn clicking_inside_the_popover_is_not_an_outside_click() {
    assert!(!is_outside_dismiss((200.0, 200.0), POPOVER, ANCHOR));
}

/// And a click genuinely elsewhere still dismisses, or the exclusion has eaten
/// the behaviour it was narrowing.
#[test]
fn clicking_elsewhere_still_dismisses() {
    assert!(is_outside_dismiss((600.0, 600.0), POPOVER, ANCHOR));
    assert!(is_outside_dismiss((120.0, 500.0), POPOVER, ANCHOR));
}

/// **The case T4.2 produces immediately:** a list of entries each with a menu
/// button. Opening a second must close the first, or there are two live
/// popovers, an ambiguous process-wide reposition counter, and a focus
/// restoration with two candidate anchors.
#[test]
fn opening_a_second_popover_dismisses_the_first() {
    assert_eq!(
        open_response(Some(PopoverId(1)), PopoverId(2)),
        OpenResponse::DismissThenOpen(PopoverId(1)),
    );
}

/// Re-opening the same one is a no-op, not a dismiss-then-open: the dismiss
/// would run the focus sequence and land focus on the anchor mid-open, which
/// reads as the menu flickering closed and back under the pointer.
#[test]
fn reopening_the_same_popover_is_a_no_op() {
    assert_eq!(
        open_response(Some(PopoverId(1)), PopoverId(1)),
        OpenResponse::AlreadyOpen,
    );
}

/// The ordinary case.
#[test]
fn opening_with_nothing_open_just_opens() {
    assert_eq!(open_response(None, PopoverId(1)), OpenResponse::Open);
}
