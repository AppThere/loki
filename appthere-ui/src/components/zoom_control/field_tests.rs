// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What opens the typed field, and what the field does with a key.

use super::{erase_last, push_digit, starts_typed_zoom};

/// A digit opens the field — the whole keyboard route to it.
#[test]
fn a_digit_starts_a_typed_zoom() {
    for c in ['0', '1', '5', '9'] {
        assert!(starts_typed_zoom(c), "{c} should start typing");
    }
}

/// **A letter does not.** Seeding the field with a mis-pressed `q` leaves the
/// reader looking at a field they now have to clear before they can do
/// anything — worse than the key having done nothing.
#[test]
fn a_letter_does_not() {
    for c in ['q', 'Z', ' ', '%', '-', '.'] {
        assert!(!starts_typed_zoom(c), "{c} should not start typing");
    }
}

/// **The buffer is built here, not read back from an element.** Typing 3, 4, 0
/// gives 340 — which is the exact case the screen sitting found broken when the
/// text lived in the DOM: it reported only the newest keystroke.
#[test]
fn typing_accumulates() {
    let mut b = String::new();
    for c in ['3', '4', '0'] {
        b = push_digit(&b, c);
    }
    assert_eq!(b, "340");
}

/// Backspace removes the last character, and is safe on an empty buffer.
#[test]
fn backspace_removes_and_never_underflows() {
    assert_eq!(erase_last("340"), "34");
    assert_eq!(erase_last(""), "");
}

/// **Capped at four characters.** The range is 20–600, so nothing longer can be
/// meant; without a cap a leaning finger fills the field with digits that then
/// clamp to 600, which looks like the field ignored most of what was typed.
#[test]
fn the_buffer_stops_growing() {
    assert_eq!(push_digit("1234", '5'), "1234");
    assert_eq!(push_digit("123", '5'), "1235");
}

/// **Only digits reach the buffer.** `push_digit` is reached through
/// `route_key`'s `Typeahead`, which carries any printable character, so the
/// guard is not redundant with the caller's — it is the one that holds if a
/// second caller ever appears.
#[test]
fn a_non_digit_never_reaches_the_buffer() {
    assert_eq!(push_digit("15", 'q'), "15");
    assert_eq!(push_digit("15", '%'), "15");
    assert_eq!(push_digit("15", '.'), "15");
}
