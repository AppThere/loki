// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::SpellChecker;

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
    let mut c = tiny_checker();
    assert!(!c.is_correct("loki"));
    c.add_word("loki").expect("add succeeds");
    assert!(c.is_correct("loki"));
}

#[test]
fn ignored_word_is_correct_case_insensitively() {
    let mut c = tiny_checker();
    assert!(!c.is_correct("xyzzy"));
    c.ignore_word("Xyzzy");
    assert!(c.is_correct("xyzzy"));
    assert!(c.is_correct("XYZZY"));
}
