// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What the zoom menu does with a key it is handed.
//!
//! Extracted from `menu.rs` at the 300-line ceiling, on a real seam: the rest of
//! that file is the popover request and the rendering, and this is the one place
//! that *decides*. It reads as a single function because it is one — the routing
//! is `route_key`'s, upstream, and everything here is what this menu's own state
//! does with the answer.

use dioxus::prelude::*;

use super::field::{erase_last, push_digit, starts_typed_zoom};
use super::menu::ZoomMenuContext;
use super::rows::{next_zoom_row, prev_zoom_row, ZoomRow};
use crate::components::popover::KeyAction;
use crate::components::zoom::parse_zoom_percent;

/// Applies `action` to the menu's state.
///
/// **No `KeyAction::Dismiss` arm anywhere below.** The host answers that one
/// itself and never forwards it, so an arm here would compile, read as handled,
/// and never run — which is exactly what happened: clearing the typed field on
/// Escape lived in a `Dismiss` arm for a release, so one Escape left the field
/// open with a stale value owning the keyboard. The reset lives in `on_dismiss`
/// now; see `KeyAction::dismissal_cause`.
pub(super) fn handle_key(
    action: KeyAction,
    ctx: &ZoomMenuContext,
    rows: &[ZoomRow],
    active: Signal<Option<String>>,
    open: Signal<bool>,
    typed: Signal<Option<String>>,
) {
    // `Signal` is `Copy`, so the captured copies are re-bound mutably here;
    // writing through a capture would need `FnMut`, which the host cannot hold.
    let mut active = active;
    let mut open = open;
    let mut typed = typed;
    // `peek`, not `read`: this runs from an event handler, and a subscription
    // taken here would tie whatever is rendering to the row the keyboard last
    // touched.
    let current = active.peek().clone();

    // While the field is open it owns the keyboard: arrows would otherwise move a
    // highlight the reader cannot see past the field, and Enter would activate a
    // preset instead of submitting.
    if typed.peek().is_some() {
        let entry = typed.peek().clone().unwrap_or_default();
        match action {
            KeyAction::Activate => {
                if let Some(p) = parse_zoom_percent(&entry) {
                    ctx.on_change.call(p);
                    typed.set(None);
                    open.set(false);
                }
                // An entry that does not parse leaves the field as it is, so the
                // reader can correct it. Closing would discard what they typed
                // and look like it was accepted.
            }
            KeyAction::Typeahead(c) if starts_typed_zoom(c) => {
                typed.set(Some(push_digit(&entry, c)));
            }
            KeyAction::Erase => typed.set(Some(erase_last(&entry))),
            _ => {}
        }
        return;
    }

    match action {
        KeyAction::Next => {
            active.set(next_zoom_row(rows, current.as_deref()).map(|r| r.key()));
        }
        KeyAction::Prev => {
            active.set(prev_zoom_row(rows, current.as_deref()).map(|r| r.key()));
        }
        KeyAction::First => active.set(rows.first().map(|r| r.key())),
        KeyAction::Last => active.set(rows.last().map(|r| r.key())),
        KeyAction::Activate => {
            if let Some(row) = current
                .as_deref()
                .and_then(|k| rows.iter().find(|r| r.key() == k))
            {
                ctx.activate(*row);
                open.set(false);
            }
        }
        // Typeahead over "25%", "50%" … would match the first character of a
        // *label*, when the character a reader types is the first digit of the
        // **value** they want. So a digit starts the typed field with that digit
        // in it — the same observation, used the other way round. See `field`.
        KeyAction::Typeahead(c) if starts_typed_zoom(c) => {
            // Every later key is routed here too — `route_key` sends the whole
            // keyboard to this closure while the menu is open — so the field's
            // buffer is edited from the same place rather than from an element
            // whose text does not survive a render.
            let entry = typed.peek().clone().unwrap_or_default();
            typed.set(Some(push_digit(&entry, c)));
        }
        _ => {}
    }
}
