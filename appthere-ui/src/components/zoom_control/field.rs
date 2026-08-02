// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The typed zoom field at the head of the zoom menu (Spec 08 T5.4).
//!
//! # Why the field is in the menu rather than in the status bar
//!
//! The bar is 24 px tall and already holds three controls; an editable readout
//! there would either overflow it — the defect r80's sitting caught — or take the
//! menu's place as the readout's click action. The menu is where a reader has
//! already gone to change the zoom, so the field costs no new chrome and no new
//! affordance to discover.
//!
//! # The keyboard route is the typeahead the menu declined to implement
//!
//! `route_key(Menu, Char(c))` produces `KeyAction::Typeahead(c)`, and the zoom
//! menu had it deliberately unhandled: typeahead over "25%", "50%" … would match
//! on the first character of a *label*, when the character a reader types is the
//! first digit of the **value** they want. That is the same observation from the
//! other side — so a digit starts the field with that digit in it, and every key
//! after that goes to the field.
//!
//! Without this the field would be pointer-only: `Role::Menu` routes `Tab` to
//! dismiss-and-advance, so there is no Tab that reaches inside the menu, and a
//! control a keyboard cannot reach is one T4.5 would have called unreachable.
//!
//! # The field must stop what it consumes
//!
//! `AtPopoverHost`'s `onkeydown` sits on an ancestor, and keys bubble to it. A
//! digit typed into the field would otherwise be routed *again* as a typeahead,
//! re-seeding the field with the character just typed. So the field stops
//! propagation for everything it handles — and deliberately **not** for Escape,
//! which belongs to the menu and must still close it.
//!
//! # The text is ours, and the element only displays it
//!
//! The obvious build is an `<input>` bound to a signal. It does not work inside
//! the popover: the content is produced by a closure the host re-invokes, the
//! element's text is re-applied on every render, and `oninput` therefore reports
//! only the newest keystroke. The first screen sitting showed it plainly —
//! seeding with `3` and typing `40` left a field containing `0`, and a probe
//! confirmed the *signal* held `0`, so this was never a rendering problem.
//!
//! So the buffer is built here from key events and the element is a plain text
//! node. That is one direction and cannot desynchronise.
//!
//! **What it costs, stated rather than discovered:** no caret, no selection, no
//! paste, and no IME. For a two-or-three digit percentage that is a fair trade;
//! for anything longer it would not be, and a field that needs those wants the
//! `<input>` path fixed instead.
//! TODO(popover-text-input): make `<input>` usable inside popover content, and
//! move this back onto it.

use dioxus::prelude::*;

use crate::tokens::colors::{COLOR_BORDER_CHROME, COLOR_TEXT_PRIMARY};
use crate::tokens::spacing::{RADIUS_SM, SPACE_1, TOUCH_MIN};
use crate::tokens::typography::FONT_SIZE_BODY;

/// Whether this character should open the typed field.
///
/// Digits only. A letter typed at a zoom menu is a mis-press, and seeding the
/// field with it would leave the reader looking at a field containing `q` that
/// they now have to clear.
#[must_use]
pub(super) fn starts_typed_zoom(c: char) -> bool {
    c.is_ascii_digit()
}

/// Appends a digit, returning the new buffer.
///
/// Capped at four characters: the range is 20–600, so nothing longer can be
/// meant, and without a cap a leaning finger fills the field with digits that
/// then clamp to 600 and look like the field ignored most of what was typed.
///
/// There is no key *classifier* here. `route_key` already turns a keyboard into
/// this menu's vocabulary, and a second classification beside it would be two
/// sources for one fact — the first draft had one, and clippy found it with no
/// caller before it could drift.
#[must_use]
pub(super) fn push_digit(buffer: &str, c: char) -> String {
    if buffer.len() < 4 && c.is_ascii_digit() {
        format!("{buffer}{c}")
    } else {
        buffer.to_string()
    }
}

/// Removes the last character. Safe on an empty buffer.
#[must_use]
pub(super) fn erase_last(buffer: &str) -> String {
    let mut b = buffer.to_string();
    b.pop();
    b
}

/// The field itself: a text node showing the buffer this module owns.
pub(super) fn typed_zoom_field(typed: Signal<Option<String>>, showing: bool) -> Element {
    let text = typed.read().clone().unwrap_or_default();
    if !showing {
        return rsx! {};
    }
    rsx! {
        div {
            style: format!(
                "margin: 0 {mh}px {mb}px {mh}px; min-height: {t}px; \
                 display: flex; align-items: center; padding: 0 {p}px; \
                 border: 1px solid {bd}; border-radius: {r}px; \
                 font-size: {fs}px; color: {fg};",
                mh = SPACE_1, mb = SPACE_1, t = TOUCH_MIN, p = SPACE_1,
                bd = COLOR_BORDER_CHROME, r = RADIUS_SM,
                fs = FONT_SIZE_BODY, fg = COLOR_TEXT_PRIMARY,
            ),
            "{text}%"
        }
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
