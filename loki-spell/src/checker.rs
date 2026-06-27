// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The [`SpellChecker`]: a dictionary plus a session ignore list.

use std::collections::HashSet;

use spellbook::Dictionary;

use crate::error::{SpellError, SpellResult};
use crate::misspelling::Misspelling;
use crate::tokenizer::tokenize;

/// A loaded spell checker for a single language.
///
/// Wraps a Hunspell-compatible [`spellbook::Dictionary`] and adds a session
/// *ignore list* (words the user chose to ignore for this session only).
/// Persistent personal-dictionary words are added with [`SpellChecker::add_word`],
/// which mutates the in-memory dictionary so they are treated as correct.
///
/// Construct one per language from the `.aff` (affix rules) and `.dic` (word
/// list) contents of a Hunspell dictionary; such dictionaries ship with
/// LibreOffice and Mozilla for essentially every locale.
pub struct SpellChecker {
    dict: Dictionary,
    /// Words ignored for this session (lower-cased for case-insensitive match).
    ignored: HashSet<String>,
}

impl SpellChecker {
    /// Builds a checker from Hunspell `.aff` and `.dic` file contents.
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::DictionaryParse`] if the affix or dictionary data
    /// is malformed.
    pub fn new(aff: &str, dic: &str) -> SpellResult<Self> {
        let dict =
            Dictionary::new(aff, dic).map_err(|e| SpellError::DictionaryParse(e.to_string()))?;
        Ok(Self {
            dict,
            ignored: HashSet::new(),
        })
    }

    /// Returns `true` if `word` is spelled correctly (or has been ignored/added).
    ///
    /// Empty input is treated as correct.
    pub fn is_correct(&self, word: &str) -> bool {
        if word.is_empty() {
            return true;
        }
        if self.ignored.contains(&word.to_lowercase()) {
            return true;
        }
        self.dict.check(word)
    }

    /// Returns ranked correction suggestions for `word`, best first.
    ///
    /// The list is empty if `word` is already correct or no suggestion is found.
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.dict.suggest(word, &mut out);
        out
    }

    /// Adds `word` to the in-memory dictionary (a persistent personal entry).
    ///
    /// The caller is responsible for persisting the user's personal word list
    /// and re-adding the words on the next load.
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::WordAdd`] if `word` cannot be parsed as a dictionary
    /// entry.
    pub fn add_word(&mut self, word: &str) -> SpellResult<()> {
        self.dict
            .add(word)
            .map_err(|e| SpellError::WordAdd(e.to_string()))
    }

    /// Ignores `word` (case-insensitively) for the remainder of the session.
    pub fn ignore_word(&mut self, word: &str) {
        self.ignored.insert(word.to_lowercase());
    }

    /// Tokenizes `text` and returns every misspelled word with its byte range.
    ///
    /// Ranges are relative to `text`; the caller maps them onto the document
    /// (e.g. paragraph + byte offset) to position squiggle decorations.
    pub fn check_text(&self, text: &str) -> Vec<Misspelling> {
        tokenize(text)
            .into_iter()
            .filter(|w| !self.is_correct(w.text))
            .map(|w| Misspelling {
                word: w.text.to_string(),
                range: w.range,
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "checker_tests.rs"]
mod tests;
