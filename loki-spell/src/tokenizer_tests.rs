// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::tokenize;

fn slices(text: &str) -> Vec<&str> {
    tokenize(text).into_iter().map(|w| w.text).collect()
}

#[test]
fn splits_on_whitespace_and_punctuation() {
    assert_eq!(
        slices("the quick, brown fox."),
        ["the", "quick", "brown", "fox"]
    );
}

#[test]
fn ranges_index_back_into_source() {
    let text = "hi teh world";
    let words = tokenize(text);
    let teh = &words[1];
    assert_eq!(teh.text, "teh");
    assert_eq!(&text[teh.range.clone()], "teh");
}

#[test]
fn keeps_internal_apostrophe() {
    assert_eq!(slices("don't can't"), ["don't", "can't"]);
    assert_eq!(slices("l'heure"), ["l'heure"]);
}

#[test]
fn keeps_curly_apostrophe() {
    assert_eq!(slices("don\u{2019}t"), ["don\u{2019}t"]);
}

#[test]
fn drops_trailing_apostrophe() {
    // Possessive plural: the trailing apostrophe is not part of the word.
    assert_eq!(slices("dogs' bones"), ["dogs", "bones"]);
}

#[test]
fn keeps_internal_hyphen() {
    assert_eq!(slices("well-known"), ["well-known"]);
}

#[test]
fn skips_tokens_with_digits() {
    assert_eq!(slices("h1 abc123 v2 word"), ["word"]);
}

#[test]
fn skips_pure_punctuation_and_numbers() {
    assert_eq!(slices("--- 42 !!! 3.14"), Vec::<&str>::new());
}

#[test]
fn handles_unicode_letters() {
    assert_eq!(slices("café naïve über"), ["café", "naïve", "über"]);
}

#[test]
fn empty_input_yields_nothing() {
    assert_eq!(slices(""), Vec::<&str>::new());
}
