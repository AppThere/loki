// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Word segmentation for spell checking.
//!
//! Splits a run of text into checkable words, returning each word together with
//! its byte range in the source string so callers can map a misspelling back to
//! a position in the document (for squiggle decorations or correction menus).
//!
//! Segmentation rules (intentionally conservative — a false *word* boundary only
//! costs a missed check, never a wrong correction applied to the document):
//!
//! - A word is a maximal run of Unicode alphanumeric characters, allowing a
//!   single apostrophe (`'`, `’`) or hyphen (`-`) as an *internal connector*
//!   when it sits directly between two alphanumeric characters (so `don't` and
//!   `l'heure` are one word, but a trailing `'` is not part of the word).
//! - Tokens containing a digit are skipped entirely (e.g. `h1`, `abc123`,
//!   version numbers) — they are rarely dictionary words and flagging them is
//!   noise.
//! - A token with no alphabetic character is skipped.

use core::ops::Range;

/// A single checkable word and its byte range within the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word<'a> {
    /// The word slice, borrowed from the source text.
    pub text: &'a str,
    /// The byte range of `text` within the source string (`source[range]`).
    pub range: Range<usize>,
}

/// Returns `true` if `c` may appear inside a word body.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

/// Returns `true` if `c` is a connector that joins two word characters.
fn is_connector(c: char) -> bool {
    matches!(c, '\'' | '\u{2019}' | '-')
}

/// Splits `text` into spell-checkable [`Word`]s with their byte ranges.
///
/// Tokens that contain a digit, or that contain no alphabetic character, are
/// omitted from the result (see the module docs for the full rule set).
pub fn tokenize(text: &str) -> Vec<Word<'_>> {
    let mut words = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut start: Option<usize> = None;
    let mut end = 0usize;

    while let Some((idx, c)) = chars.next() {
        let char_len = c.len_utf8();
        if is_word_char(c) {
            if start.is_none() {
                start = Some(idx);
            }
            end = idx + char_len;
        } else if is_connector(c) && start.is_some() {
            // Keep the connector only if the next char is also a word char.
            let next_is_word = chars
                .peek()
                .map(|&(_, nc)| is_word_char(nc))
                .unwrap_or(false);
            if next_is_word {
                end = idx + char_len;
            } else {
                push_word(text, &mut words, start.take(), end);
            }
        } else if start.is_some() {
            push_word(text, &mut words, start.take(), end);
        }
    }
    push_word(text, &mut words, start, end);
    words
}

/// Pushes the `[start, end)` slice as a [`Word`] unless it should be skipped.
fn push_word<'a>(text: &'a str, words: &mut Vec<Word<'a>>, start: Option<usize>, end: usize) {
    let Some(start) = start else { return };
    if start >= end {
        return;
    }
    let slice = &text[start..end];
    let mut has_alpha = false;
    for c in slice.chars() {
        if c.is_numeric() {
            return; // token mixes in a digit — skip it entirely
        }
        if c.is_alphabetic() {
            has_alpha = true;
        }
    }
    if has_alpha {
        words.push(Word {
            text: slice,
            range: start..end,
        });
    }
}

#[cfg(test)]
#[path = "tokenizer_tests.rs"]
mod tests;
