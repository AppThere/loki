// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The platform-to-vocabulary step, which is where a stack's quirks arrive.
//!
//! `route_key` has been testable since it was written; this is the translation
//! in front of it, and a mistake here makes `route_key` correct and unreachable
//! — an instrument in the right place, reporting on an input that never comes.

use dioxus::prelude::{Key as DioxusKey, Modifiers};

use super::key_from_parts;
use crate::components::popover::Key;

/// A key press with no modifiers.
fn press(key: DioxusKey) -> Option<Key> {
    key_from_parts(&key, Modifiers::empty())
}

/// A key press with `mods` held.
fn press_with(key: DioxusKey, mods: Modifiers) -> Option<Key> {
    key_from_parts(&key, mods)
}

#[test]
fn the_navigation_keys_map_to_the_vocabulary() {
    for (from, to) in [
        (DioxusKey::ArrowDown, Key::Down),
        (DioxusKey::ArrowUp, Key::Up),
        (DioxusKey::Home, Key::Home),
        (DioxusKey::End, Key::End),
        (DioxusKey::Enter, Key::Activate),
        (DioxusKey::Escape, Key::Escape),
        (DioxusKey::Tab, Key::Tab),
    ] {
        assert_eq!(press(from.clone()), Some(to), "{from:?}");
    }
}

/// **Shift+Tab is `Tab` plus a modifier, not its own key.** Reading only the key
/// would make Shift+Tab close a menu and advance focus *forward*, which is the
/// one thing a back-tab must not do.
#[test]
fn shift_tab_is_distinguished_from_tab() {
    assert_eq!(
        press_with(DioxusKey::Tab, Modifiers::SHIFT),
        Some(Key::ShiftTab),
    );
    assert_eq!(press(DioxusKey::Tab), Some(Key::Tab));
}

/// **Space arrives as a one-character string, not as a named key.** A menu row
/// is a button and a button takes Space; without the special case it would
/// become a typeahead for `" "`, which matches no label ever.
#[test]
fn space_activates_rather_than_becoming_a_typeahead() {
    assert_eq!(press(DioxusKey::Character(" ".into())), Some(Key::Activate));
}

/// An ordinary letter is a typeahead.
#[test]
fn a_printable_character_is_a_typeahead() {
    assert_eq!(
        press(DioxusKey::Character("d".into())),
        Some(Key::Char('d'))
    );
}

/// **Ctrl, Alt and Meta belong to the application, not to the menu.** Ctrl+C
/// with a menu open is a copy; treating it as a typeahead for "c" would move the
/// selection and swallow the shortcut.
#[test]
fn a_modified_key_is_not_ours() {
    for mods in [Modifiers::CONTROL, Modifiers::ALT, Modifiers::META] {
        assert_eq!(
            press_with(DioxusKey::Character("c".into()), mods),
            None,
            "{mods:?}+C must reach the application",
        );
        // Escape too: Ctrl+Escape is a system gesture on some platforms.
        assert_eq!(press_with(DioxusKey::Escape, mods), None);
    }
}

/// A multi-character `Character` is an IME composition or a dead-key sequence —
/// text input, which belongs to whatever has focus rather than to a typeahead.
#[test]
fn a_composed_sequence_is_not_a_typeahead() {
    assert_eq!(press(DioxusKey::Character("shi".into())), None);
}

/// **The polarity that keeps every `None` above meaningful (L08-045).** Without
/// a key that maps, `key_from_parts` returning `None` always satisfies the
/// modifier and composition tests — and the popover would be deaf while every
/// assertion passed.
#[test]
fn something_maps_or_the_none_cases_prove_nothing() {
    assert!(press(DioxusKey::Escape).is_some());
    assert!(press(DioxusKey::ArrowDown).is_some());
}

/// A key outside the vocabulary is `None` rather than a guess.
#[test]
fn an_unmodelled_key_is_none() {
    assert_eq!(press(DioxusKey::F5), None);
}
