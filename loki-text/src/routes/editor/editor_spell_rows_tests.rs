// SPDX-License-Identifier: Apache-2.0

//! The spelling menu's row table and the walk over it.

use super::super::editor_spell::SpellMenu;
use super::{SpellRow, next_spell_row, prev_spell_row, spell_rows};

fn menu(misspelled: bool, suggestions: &[&str]) -> SpellMenu {
    SpellMenu {
        paragraph_index: 0,
        byte_start: 0,
        byte_end: 4,
        word: "teh".to_string(),
        misspelled,
        suggestions: suggestions.iter().map(|s| (*s).to_string()).collect(),
        anchor_x: 100.0,
        anchor_y: 100.0,
    }
}

/// The ordinary case: suggestions first, then the two word actions, then the
/// language row.
#[test]
fn a_misspelled_word_lists_suggestions_then_actions() {
    assert_eq!(
        spell_rows(&menu(true, &["the", "ten"])),
        vec![
            SpellRow::Suggestion(0),
            SpellRow::Suggestion(1),
            SpellRow::Add,
            SpellRow::Ignore,
            SpellRow::Language,
        ],
    );
}

/// **A correctly-spelled word has exactly one row.** Right-clicking a word that
/// is spelled fine still opens the menu — the renderer draws only the language
/// row — and a navigator that offered Add or Ignore there would let Enter add a
/// correct word to the dictionary.
#[test]
fn a_correct_word_offers_only_the_language_row() {
    assert_eq!(spell_rows(&menu(false, &[])), vec![SpellRow::Language]);
    assert_eq!(
        spell_rows(&menu(false, &["the"])),
        vec![SpellRow::Language],
        "suggestions are not drawn for a correct word, so they are not walkable",
    );
}

/// A misspelled word with no suggestions still has its actions.
#[test]
fn no_suggestions_still_leaves_the_actions() {
    assert_eq!(
        spell_rows(&menu(true, &[])),
        vec![SpellRow::Add, SpellRow::Ignore, SpellRow::Language],
    );
}

/// Keys are stable and distinct — they are what the pointer and the keyboard
/// agree on, so a collision would highlight two rows at once.
#[test]
fn every_row_has_its_own_key() {
    let rows = spell_rows(&menu(true, &["the", "ten", "tea"]));
    let mut keys: Vec<String> = rows.iter().map(|r| r.key()).collect();
    let before = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), before, "duplicate row key in {keys:?}");
    assert_eq!(SpellRow::Suggestion(2).key(), "sug-2");
}

/// The first press enters from the end the user expects, in both directions.
#[test]
fn the_first_press_enters_from_the_right_end() {
    let rows = spell_rows(&menu(true, &["the"]));
    assert_eq!(next_spell_row(&rows, None), Some(SpellRow::Suggestion(0)));
    assert_eq!(prev_spell_row(&rows, None), Some(SpellRow::Language));
}

/// Down walks forward and wraps.
#[test]
fn down_walks_forward_and_wraps() {
    let rows = spell_rows(&menu(true, &["the"]));
    assert_eq!(next_spell_row(&rows, Some("sug-0")), Some(SpellRow::Add));
    assert_eq!(next_spell_row(&rows, Some("add")), Some(SpellRow::Ignore));
    assert_eq!(
        next_spell_row(&rows, Some("lang")),
        Some(SpellRow::Suggestion(0)),
        "past the last row is the first",
    );
}

/// Up walks backward and wraps — the polarity of the test above (L08-045),
/// without which `prev_spell_row` could be `next_spell_row` and go unnoticed.
#[test]
fn up_walks_backward_and_wraps() {
    let rows = spell_rows(&menu(true, &["the"]));
    assert_eq!(prev_spell_row(&rows, Some("lang")), Some(SpellRow::Ignore));
    assert_eq!(
        prev_spell_row(&rows, Some("add")),
        Some(SpellRow::Suggestion(0))
    );
    assert_eq!(
        prev_spell_row(&rows, Some("sug-0")),
        Some(SpellRow::Language)
    );
}

/// **The case the pointer creates.** `spell_hover` survives the menu re-opening
/// on a different word, so it can hold `sug-3` when the new word has one
/// suggestion. That key names no row, and the arrow must land at an end rather
/// than returning `None` — a menu where the first arrow press does nothing
/// reads as a menu with no keyboard at all.
#[test]
fn a_stale_hover_key_does_not_strand_the_keyboard() {
    let rows = spell_rows(&menu(true, &["the"]));
    assert_eq!(
        next_spell_row(&rows, Some("sug-3")),
        Some(SpellRow::Suggestion(0)),
    );
    assert_eq!(
        prev_spell_row(&rows, Some("sug-3")),
        Some(SpellRow::Language)
    );
}

/// Down and Up are inverses at every row, including across the wrap.
#[test]
fn down_then_up_returns_to_the_same_row() {
    let rows = spell_rows(&menu(true, &["the", "ten"]));
    for row in &rows {
        let key = row.key();
        let down = next_spell_row(&rows, Some(&key)).expect("non-empty");
        assert_eq!(prev_spell_row(&rows, Some(&down.key())), Some(*row));
    }
}

/// A menu always has at least one row, so the walk can never return `None` for a
/// real menu. This is what makes the `Option` at the call site a total function
/// in practice rather than a case nobody has thought about.
#[test]
fn the_walk_always_finds_a_row() {
    for misspelled in [true, false] {
        for suggestions in [&[][..], &["the"][..], &["the", "ten"][..]] {
            let rows = spell_rows(&menu(misspelled, suggestions));
            assert!(!rows.is_empty(), "{misspelled}/{suggestions:?}");
            assert!(next_spell_row(&rows, None).is_some());
            assert!(prev_spell_row(&rows, None).is_some());
        }
    }
}
