// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Keyboard navigation over the Recent menu's rows.
//!
//! `activate_row` and `menu_content` both need a Dioxus runtime (an
//! `EventHandler` cannot be constructed without one), so what is covered here is
//! the part they both consult: the row table and the two movement functions.
//! That is deliberate rather than a gap — [`super::action_for_row`] is what
//! decides *which* action an index runs, and it is the thing a wrong index would
//! get wrong.

use super::{action_for_row, next_row, prev_row, MenuAction, ROWS, ROW_COUNT};

/// **The divergence this module exists to make impossible.** The rendered order
/// and the keyboard's order are the same array, so this reads as a tautology —
/// which is the point. It fails the moment somebody reintroduces a second list.
///
/// Delete is row 1 in both, which is the entry that makes a divergence expensive.
#[test]
fn the_rendered_order_is_the_order_the_keyboard_walks() {
    assert_eq!(action_for_row(0), Some(MenuAction::Remove));
    assert_eq!(action_for_row(1), Some(MenuAction::Delete));
    assert_eq!(action_for_row(2), Some(MenuAction::OpenCopy));
    assert_eq!(ROW_COUNT, ROWS.len());
}

/// An index past the end names no action rather than wrapping into one. A clamp
/// here would run `OpenCopy`; a modulo would run `Remove`. Both are silent, and
/// on a menu whose middle row deletes a file, silence is the wrong failure.
#[test]
fn an_out_of_range_row_names_no_action() {
    assert_eq!(action_for_row(ROW_COUNT), None);
    assert_eq!(action_for_row(usize::MAX), None);
}

/// **The first press has to land somewhere a user can predict.** Down goes to
/// the top of the menu and Up goes to the bottom, so either key reaches a row in
/// one press. Starting Down at row 1 — the shape you get by writing
/// `current.unwrap_or(0) + 1` — silently skips `Remove from recents`.
#[test]
fn the_first_press_enters_the_menu_from_the_right_end() {
    assert_eq!(next_row(None), 0, "Down enters at the first row");
    assert_eq!(prev_row(None), ROW_COUNT - 1, "Up enters at the last row");
}

/// Down walks forward and wraps at the end.
#[test]
fn down_walks_forward_and_wraps() {
    assert_eq!(next_row(Some(0)), 1);
    assert_eq!(next_row(Some(1)), 2);
    assert_eq!(
        next_row(Some(ROW_COUNT - 1)),
        0,
        "past the last row is the first, not a row that does not exist",
    );
}

/// Up walks backward and wraps at the start — the polarity of the test above
/// (L08-045). Without it, `prev_row` returning `next_row`'s answer everywhere
/// would still pass every forward assertion.
#[test]
fn up_walks_backward_and_wraps() {
    assert_eq!(prev_row(Some(2)), 1);
    assert_eq!(prev_row(Some(1)), 0);
    assert_eq!(prev_row(Some(0)), ROW_COUNT - 1);
}

/// **The two are inverses**, which is the property a user actually relies on:
/// Down-then-Up returns to where they were, at every row including the wrap.
/// Checked as a round trip rather than as a table, so it keeps holding when a
/// fourth action is added.
#[test]
fn down_then_up_returns_to_the_same_row() {
    for row in 0..ROW_COUNT {
        assert_eq!(
            prev_row(Some(next_row(Some(row)))),
            row,
            "row {row} down-up"
        );
        assert_eq!(
            next_row(Some(prev_row(Some(row)))),
            row,
            "row {row} up-down"
        );
    }
}

/// Every row the movement functions can produce is a real action. This is what
/// makes `activate_row`'s `None` arm unreachable in practice rather than merely
/// handled — and it is the assertion that would fail if `ROW_COUNT` were ever
/// written down separately from `ROWS` again.
#[test]
fn every_reachable_row_has_an_action() {
    let mut row = None;
    for _ in 0..(2 * ROW_COUNT + 1) {
        let next = next_row(row);
        assert!(action_for_row(next).is_some(), "Down reached row {next}");
        let back = prev_row(row);
        assert!(action_for_row(back).is_some(), "Up reached row {back}");
        row = Some(next);
    }
}
