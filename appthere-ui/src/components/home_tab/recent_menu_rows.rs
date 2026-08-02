// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The Recent menu's rows: what they are, how the keyboard moves between them,
//! and how they render (Spec 08 T4.2/T4.5).
//!
//! # One table, so the pointer and the keyboard cannot disagree
//!
//! The rows were three hand-written `button` elements and the keyboard's
//! `Activate` was a `match row { 0 => .., 1 => .., 2 => .. }`. That is two
//! sources for one fact (L08-029) with the worst possible failure: row 1 is
//! **Delete file**, so a divergence between the two orders means Enter deletes a
//! document the user was pointing at `Remove from recents` to keep. Nothing about
//! the divergence would be visible — both lists read correctly on their own.
//!
//! So [`ROWS`] is the single order, the buttons are built by iterating it, and
//! the keyboard indexes it. Adding an action is one edit, and the row count
//! follows from the array rather than from a constant somebody remembers to bump.

use std::rc::Rc;

use dioxus::prelude::*;

use super::RecentMenuActions;
use crate::tokens::colors::{
    COLOR_BORDER_CHROME, COLOR_STATUS_ERROR_TEXT, COLOR_SURFACE_PAGE, COLOR_TEXT_PRIMARY,
};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_1, SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::FONT_SIZE_BODY;

/// Background of the row the keyboard is on.
///
/// The menu's own surface is `COLOR_SURFACE_PAGE` (light), so the tint is a light
/// one — the `COLOR_*_HOVER` tokens are all chrome-dark and would read as a hole
/// rather than a highlight here. A literal because the token set has no
/// light-surface hover yet.
/// TODO(tokens-light-hover): add one and use it.
const ACTIVE_ROW_BG: &str = "#EDEDED";

/// One row of the menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum MenuAction {
    /// Drop the entry from the recents list, leaving the file alone.
    Remove,
    /// Delete the file on disk — the destructive one.
    Delete,
    /// Open a copy, leaving the original untouched.
    OpenCopy,
}

/// The menu's rows, in the order they are shown **and** the order the keyboard
/// walks. See the module docs for why there is exactly one of these.
pub(super) const ROWS: [MenuAction; 3] =
    [MenuAction::Remove, MenuAction::Delete, MenuAction::OpenCopy];

/// How many rows the menu has — from the array, so it cannot drift from it.
pub(super) const ROW_COUNT: usize = ROWS.len();

impl MenuAction {
    /// This row's label.
    fn label(self, actions: &RecentMenuActions) -> String {
        match self {
            Self::Remove => actions.remove_label.clone(),
            Self::Delete => actions.delete_label.clone(),
            Self::OpenCopy => actions.open_copy_label.clone(),
        }
    }

    /// This row's text colour. Delete is the only destructive action and reads
    /// as one.
    fn color(self) -> &'static str {
        match self {
            Self::Delete => COLOR_STATUS_ERROR_TEXT,
            Self::Remove | Self::OpenCopy => COLOR_TEXT_PRIMARY,
        }
    }

    /// The handler this row calls.
    fn handler(self, actions: &RecentMenuActions) -> EventHandler<usize> {
        match self {
            Self::Remove => actions.on_remove,
            Self::Delete => actions.on_delete,
            Self::OpenCopy => actions.on_open_copy,
        }
    }
}

/// The action at `row`, or `None` when the index names no row.
///
/// `Option` rather than a wrap or a clamp: an out-of-range row is a caller
/// mistake, and both alternatives would *silently* run some other action — which
/// on this menu means deleting a file because an index was wrong.
#[must_use]
pub(super) fn action_for_row(row: usize) -> Option<MenuAction> {
    ROWS.get(row).copied()
}

/// The row after `current`, wrapping. `None` — no keyboard selection yet — goes
/// to the first row, so the first Down lands on row 0 rather than row 1.
#[must_use]
pub(super) fn next_row(current: Option<usize>) -> usize {
    match current {
        Some(row) => (row + 1) % ROW_COUNT,
        None => 0,
    }
}

/// The row before `current`, wrapping. `None` goes to the **last** row, so the
/// first Up reaches the bottom of the menu in one press.
#[must_use]
pub(super) fn prev_row(current: Option<usize>) -> usize {
    match current {
        Some(0) | None => ROW_COUNT - 1,
        Some(row) => row - 1,
    }
}

/// Runs the action on `row`, **closing first**.
///
/// Two of the three actions change the list the menu is anchored inside —
/// removing an entry moves every row below it — so a menu left open would be
/// attached to a different document by the identity check's own definition.
///
/// `dismiss` is the popover's cause-carrying dismissal rather than the parent's
/// state-clearing handler, so choosing a row returns focus to the ⋮ button the
/// way Escape does. Taking it as an `Rc<dyn Fn()>` is what lets the click path
/// and the key path share this function (T4.5).
pub(super) fn activate_row(
    row: usize,
    actions: &RecentMenuActions,
    index: usize,
    dismiss: &Rc<dyn Fn()>,
) {
    let Some(action) = action_for_row(row) else {
        return;
    };
    dismiss();
    action.handler(actions).call(index);
}

/// The menu's rows.
///
/// `active` is the row the keyboard is on, `None` when the menu was opened with
/// the pointer and no key has arrived. It is passed in rather than read here so
/// the read happens inside the host's render and subscribes the host.
pub(super) fn menu_content(
    actions: RecentMenuActions,
    index: usize,
    dismiss: &Rc<dyn Fn()>,
    active: Option<usize>,
) -> Element {
    let row_style = move |action: MenuAction, row: usize| {
        format!(
            "background: {bg}; border: none; text-align: left; width: 100%; \
             padding: {p}px {ph}px; min-height: {touch}px; cursor: pointer; \
             font-size: {size}px; color: {fg}; border-radius: {r}px; \
             box-sizing: border-box;",
            // The keyboard's position must be *visible*, or arrow keys move an
            // invisible cursor and Enter fires something the user cannot see.
            bg = if active == Some(row) {
                ACTIVE_ROW_BG
            } else {
                "transparent"
            },
            fg = action.color(),
            p = SPACE_2,
            ph = SPACE_3,
            touch = TOUCH_MIN,
            size = FONT_SIZE_BODY,
            r = RADIUS_SM,
        )
    };
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px; \
                 width: 100%; box-sizing: border-box; padding: {p}px; \
                 background: {bg}; border: 1px solid {border}; \
                 border-radius: {r}px;",
                gap = SPACE_1,
                p = SPACE_2,
                bg = COLOR_SURFACE_PAGE,
                border = COLOR_BORDER_CHROME,
                r = RADIUS_MD,
            ),
            for (row , action) in ROWS.iter().copied().enumerate() {
                button {
                    key: "{row}",
                    style: row_style(action, row),
                    // The same function the keyboard's Activate calls, so the
                    // two cannot drift.
                    onclick: {
                        let actions = actions.clone();
                        let dismiss = Rc::clone(dismiss);
                        move |_| activate_row(row, &actions, index, &dismiss)
                    },
                    "{action.label(&actions)}"
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "recent_menu_rows_tests.rs"]
mod tests;
