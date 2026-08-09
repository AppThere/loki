// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Translating a platform key event into the primitive's [`Key`] (Spec 08 T4.5).
//!
//! # A separate decision, because it is where the platform gets in
//!
//! [`route_key`](super::route_key) is a table over *our* vocabulary and has been
//! testable since it was written. What was missing is the step before it: which
//! `dioxus_html::Key` and modifier combination is which of ours. That step is
//! where a stack's quirks arrive — Space arriving as `Character(" ")` rather than
//! as a named key, Shift+Tab arriving as `Tab` with a modifier rather than as its
//! own key — and a mistake in it makes `route_key` correct and unreachable.
//!
//! So it is a pure function with its own tests rather than a `match` inside the
//! host's handler, on the same rule the rest of this module follows: the
//! component wires, it does not decide.

use dioxus::prelude::{Key as DioxusKey, Modifiers};

use super::Key;

/// Maps a key press to the primitive's vocabulary, or `None` when it is not a
/// key the popover has an opinion about.
///
/// `None` is not the same as [`super::KeyAction::PassThrough`]: this says "not in
/// our vocabulary", and `route_key` says "in our vocabulary and not ours to
/// handle". Collapsing them would make a panel's `ArrowDown` — which *is* ours to
/// deliberately pass on — indistinguishable from a key we never modelled.
/// Takes the two parts rather than the event, so it is pure over plain values —
/// the host does the one-line extraction. A `KeyboardData` cannot be constructed
/// in a unit test without a runtime, and a decision that can only be exercised
/// through the component is one the component is deciding.
#[must_use]
pub fn key_from_parts(key: &DioxusKey, mods: Modifiers) -> Option<Key> {
    // Modifiers other than Shift are the platform's or the application's:
    // Ctrl+C inside an open menu is a copy, not a typeahead for "c".
    if mods.ctrl() || mods.alt() || mods.meta() {
        return None;
    }
    match key {
        DioxusKey::ArrowDown => Some(Key::Down),
        DioxusKey::ArrowUp => Some(Key::Up),
        DioxusKey::Home => Some(Key::Home),
        DioxusKey::End => Some(Key::End),
        DioxusKey::Enter => Some(Key::Activate),
        DioxusKey::Escape => Some(Key::Escape),
        DioxusKey::Backspace => Some(Key::Backspace),
        DioxusKey::Tab if mods.shift() => Some(Key::ShiftTab),
        DioxusKey::Tab => Some(Key::Tab),
        // Space arrives as a one-character string rather than as a named key,
        // and it activates — a menu row is a button, and a button takes Space.
        // Checked before the general character case, or Space would become a
        // typeahead for " " and match nothing forever.
        DioxusKey::Character(s) if s == " " => Some(Key::Activate),
        DioxusKey::Character(s) => {
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                // Exactly one character. A multi-character `Character` is an IME
                // composition or a dead-key sequence, which is text input rather
                // than a typeahead and belongs to whatever has focus.
                (Some(c), None) => Some(Key::Char(c)),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "key_event_tests.rs"]
mod tests;
