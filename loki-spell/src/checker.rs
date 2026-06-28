// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The [`SpellChecker`]: a dictionary plus a session ignore list.

use std::collections::HashSet;
use std::sync::{PoisonError, RwLock};

use spellbook::Dictionary;

use crate::error::{SpellError, SpellResult};
use crate::misspelling::Misspelling;
use crate::tokenizer::tokenize;

/// User-supplied overrides layered on top of the dictionary.
///
/// Held behind a lock so they can be updated through a shared `&self` (the
/// checker is shared as `Arc<SpellChecker>` with the layout engine), making
/// "Add to dictionary" / "Ignore" take effect for live squiggles without
/// rebuilding the dictionary. Both sets are lower-cased for case-insensitive
/// matching.
#[derive(Debug, Default)]
struct Overrides {
    /// Words ignored for this session.
    ignored: HashSet<String>,
    /// Personal-dictionary words added by the user (process-lifetime; the host
    /// persists and re-adds them across sessions).
    personal: HashSet<String>,
}

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
///
/// A `SpellChecker` is read-only after construction (apart from `add_word` /
/// `ignore_word`) and is `Send + Sync`, so it can be shared across threads
/// behind an `Arc` — e.g. handed to the layout engine for squiggle emission
/// while the UI thread queries suggestions.
#[derive(Debug)]
pub struct SpellChecker {
    dict: Dictionary,
    /// User overrides (ignore list + personal words), updatable through `&self`.
    overrides: RwLock<Overrides>,
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
            overrides: RwLock::new(Overrides::default()),
        })
    }

    /// Builds a checker from the dictionary bundled in the crate (`en`).
    ///
    /// Always available offline; used as the first-run default and the ultimate
    /// fallback when the user's locale has no installed dictionary.
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::DictionaryParse`] only if the embedded dictionary
    /// is corrupt (a build-time bug).
    pub fn bundled() -> SpellResult<Self> {
        let (aff, dic) = crate::bundled::bundled_dictionary();
        Self::new(aff, dic)
    }

    /// Builds a checker for an installed `tag` from a [`DictionaryStore`].
    ///
    /// # Errors
    ///
    /// Returns [`SpellError::NotInstalled`] if the tag is absent, or a load /
    /// parse error otherwise.
    pub fn from_store(store: &crate::store::DictionaryStore, tag: &str) -> SpellResult<Self> {
        let (aff, dic) = store.load(tag)?;
        Self::new(&aff, &dic)
    }

    /// Returns `true` if `word` is spelled correctly (or has been ignored/added).
    ///
    /// Empty input is treated as correct.
    pub fn is_correct(&self, word: &str) -> bool {
        if word.is_empty() {
            return true;
        }
        if self.dict.check(word) {
            return true;
        }
        let lower = word.to_lowercase();
        let overrides = self
            .overrides
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        overrides.personal.contains(&lower) || overrides.ignored.contains(&lower)
    }

    /// Returns ranked correction suggestions for `word`, best first.
    ///
    /// The list is empty if `word` is already correct or no suggestion is found.
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut out = Vec::new();
        self.dict.suggest(word, &mut out);
        out
    }

    /// Adds `word` to the user's personal dictionary (case-insensitive).
    ///
    /// Takes `&self` (interior mutability) so it works through the shared
    /// `Arc<SpellChecker>` held by the layout engine. Matching is exact /
    /// case-insensitive — affix expansion is not applied to personal words. The
    /// host is responsible for persisting the personal list and re-adding it on
    /// the next load.
    pub fn add_word(&self, word: &str) {
        self.overrides
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .personal
            .insert(word.to_lowercase());
    }

    /// Ignores `word` (case-insensitively) for the remainder of the session.
    ///
    /// Takes `&self` for the same shared-`Arc` reason as [`Self::add_word`].
    pub fn ignore_word(&self, word: &str) {
        self.overrides
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .ignored
            .insert(word.to_lowercase());
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
