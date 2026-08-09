// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The wiring decisions, asserted as the failures a user would hit.

use super::super::geometry::Rect;
use super::{dismiss_on_unmount, is_outside_dismiss, open_response, OpenResponse, PopoverId};

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

use super::{on_anchor_identity, AnchorKey, IdentityCheck};

/// **The failure the geometric comparison reports as `Ignore`:** a virtualised
/// list recycles a different row into the same node, the anchor rect never
/// moves, and the open menu now acts on the wrong document.
#[test]
fn a_recycled_row_dismisses_even_though_the_rect_did_not_move() {
    let check = on_anchor_identity(AnchorKey(7), Some(AnchorKey(9)));
    assert_eq!(check, IdentityCheck::Recycled);
    assert!(
        check.must_dismiss(),
        "a menu left open over a recycled row acts on the wrong document",
    );
}

/// An emptied node is the same class — the commands have no target.
#[test]
fn an_emptied_anchor_dismisses() {
    let check = on_anchor_identity(AnchorKey(7), None);
    assert_eq!(check, IdentityCheck::Gone);
    assert!(check.must_dismiss());
}

/// The ordinary case must not dismiss, or a scrolling list closes its own menu
/// on every frame.
#[test]
fn the_same_target_carries_on() {
    let check = on_anchor_identity(AnchorKey(7), Some(AnchorKey(7)));
    assert_eq!(check, IdentityCheck::Same);
    assert!(!check.must_dismiss());
}

/// **The defect that killed the whole application.** `SpellPopover` is mounted
/// behind `if spell_menu.read().is_some()`. Choosing a suggestion set that to
/// `None`, the consumer unmounted, and the `use_effect` that was the only caller
/// of `dismiss` went with it — so `open` stayed `Some` and the host kept
/// rendering its window-sized backdrop, with nothing visible inside it. Every
/// click in the application landed on that backdrop from then on.
#[test]
fn the_open_popovers_consumer_unmounting_closes_it() {
    assert!(
        dismiss_on_unmount(Some(PopoverId(1)), PopoverId(1)),
        "the consumer that owns the open popover went away, so nothing is left \
         to close it — leaving the host's backdrop over the whole application",
    );
}

/// The polarity that makes the assertion above mean something (L08-045): keyed
/// on the id, so it is not simply "always dismiss".
///
/// The case is real, not hypothetical — [`OpenResponse::DismissThenOpen`] means
/// B can be open while A's consumer is still mounted, and A unmounting must not
/// take B down with it. An unkeyed cleanup would produce a menu that vanishes
/// because an unrelated component went away.
#[test]
fn a_different_popovers_consumer_unmounting_leaves_it_alone() {
    assert!(
        !dismiss_on_unmount(Some(PopoverId(2)), PopoverId(1)),
        "popover 2 is open and 1's consumer unmounted — closing 2 here would be \
         a menu disappearing for a reason the user cannot see",
    );
}

/// The third polarity: nothing open is not a dismissal either. Without this the
/// predicate could be `currently_open.is_none() || ...` and both tests above
/// would still pass.
#[test]
fn unmounting_with_nothing_open_is_not_a_dismissal() {
    assert!(!dismiss_on_unmount(None, PopoverId(1)));
}
