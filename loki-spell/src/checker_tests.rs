// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::SpellChecker;

fn _assert_send_sync<T: Send + Sync>() {}

#[test]
fn checker_is_send_sync() {
    // Required so an `Arc<SpellChecker>` can be shared with the layout engine
    // and across the app's worker threads.
    _assert_send_sync::<SpellChecker>();
}

/// A minimal in-memory Hunspell dictionary for tests (no affix rules).
fn tiny_checker() -> SpellChecker {
    let aff = "SET UTF-8\n";
    let dic = "4\nhello\nworld\nquick\nfox\n";
    SpellChecker::new(aff, dic).expect("tiny dictionary parses")
}

#[test]
fn rejects_invalid_dictionary() {
    // A `.dic` whose count line is not a number is a parse error.
    let err = SpellChecker::new("SET UTF-8\n", "not-a-count\nhello\n");
    assert!(err.is_err());
}

#[test]
fn known_words_are_correct() {
    let c = tiny_checker();
    assert!(c.is_correct("hello"));
    assert!(c.is_correct("world"));
    assert!(c.is_correct("")); // empty is vacuously correct
}

#[test]
fn unknown_words_are_incorrect() {
    let c = tiny_checker();
    assert!(!c.is_correct("teh"));
    assert!(!c.is_correct("xyzzy"));
}

#[test]
fn check_text_reports_ranges() {
    let c = tiny_checker();
    let text = "hello teh world";
    let misspellings = c.check_text(text);
    assert_eq!(misspellings.len(), 1);
    assert_eq!(misspellings[0].word, "teh");
    assert_eq!(&text[misspellings[0].range.clone()], "teh");
}

#[test]
fn suggest_offers_near_miss() {
    let c = tiny_checker();
    let suggestions = c.suggest("helo");
    assert!(
        suggestions.iter().any(|s| s == "hello"),
        "expected 'hello' among {suggestions:?}"
    );
}

#[test]
fn added_word_becomes_correct() {
    let c = tiny_checker();
    assert!(!c.is_correct("loki"));
    c.add_word("loki");
    assert!(c.is_correct("loki"));
    // Case-insensitive, and through a shared reference (as the layout engine
    // holds it).
    let shared = std::sync::Arc::new(c);
    assert!(shared.is_correct("LOKI"));
}

#[test]
fn ignored_word_is_correct_case_insensitively() {
    let c = tiny_checker();
    assert!(!c.is_correct("xyzzy"));
    c.ignore_word("Xyzzy");
    assert!(c.is_correct("xyzzy"));
    assert!(c.is_correct("XYZZY"));
}

#[test]
fn bundled_dictionary_loads_and_checks_real_words() {
    let c = SpellChecker::bundled().expect("bundled en dictionary parses");
    assert!(c.is_correct("hello"));
    assert!(c.is_correct("dictionary"));
    assert!(!c.is_correct("teh"));
    assert!(
        c.suggest("teh").iter().any(|s| s == "the"),
        "expected 'the' as a suggestion for 'teh'"
    );
}

#[test]
fn personal_words_lists_added_words_sorted_without_ignores() {
    let c = tiny_checker();
    c.add_word("Zebra");
    c.add_word("apple");
    c.ignore_word("session-only");
    assert_eq!(
        c.personal_words(),
        vec!["apple".to_string(), "zebra".to_string()],
        "lowercased, sorted, ignores excluded"
    );
}
