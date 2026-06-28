// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Hunspell-compatible spell checking for the Loki suite.
//!
//! `loki-spell` wraps the pure-Rust [`spellbook`] engine (a Rust port of
//! Nuspell) behind a small, document-oriented API. It owns no UI, rendering, or
//! format concerns — callers feed it text and receive misspelled byte ranges
//! and ranked suggestions.
//!
//! # Pipeline
//!
//! 1. Load a [`SpellChecker`] — from the bundled `en` dictionary
//!    ([`SpellChecker::bundled`]), an installed one ([`SpellChecker::from_store`]),
//!    or raw Hunspell `.aff` + `.dic` data ([`SpellChecker::new`]).
//! 2. Call [`SpellChecker::check_text`] on a run of text (e.g. a paragraph) to
//!    get a [`Misspelling`] per flagged word, each carrying the word's byte
//!    range so the renderer can position a squiggle decoration.
//! 3. Offer [`SpellChecker::suggest`] corrections in a context menu, and let the
//!    user [`SpellChecker::add_word`] (personal dictionary) or
//!    [`SpellChecker::ignore_word`] (this session).
//!
//! # Dictionaries
//!
//! [`Catalog`] enumerates available languages with their licenses and download
//! sources. A permissive `en` dictionary is bundled (see [`bundled`]); other
//! languages are downloaded on demand and cached in a [`DictionaryStore`].
//! Downloads run through [`fetch::install_dictionary`], which enforces a license
//! [`Consent`] gate and SHA-256 integrity before installing. Use [`locale`] to
//! resolve a host locale (e.g. `en-US`) to the best available entry.
//!
//! # Example
//!
//! ```no_run
//! use loki_spell::SpellChecker;
//!
//! # fn load() -> (String, String) { (String::new(), String::new()) }
//! let (aff, dic) = load(); // read en_US.aff / en_US.dic
//! let checker = SpellChecker::new(&aff, &dic).expect("valid dictionary");
//! for miss in checker.check_text("teh quick brown fox") {
//!     let suggestions = checker.suggest(&miss.word);
//!     println!("{:?} at {:?} -> {:?}", miss.word, miss.range, suggestions);
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod bundled;
pub mod catalog;
pub mod checker;
pub mod error;
pub mod fetch;
pub mod license;
pub mod locale;
pub mod misspelling;
pub mod store;
pub mod tokenizer;

pub use catalog::{Catalog, DictionaryEntry, DictionarySource};
pub use checker::SpellChecker;
pub use error::{SpellError, SpellResult};
pub use fetch::{install_dictionary, DictionaryFetcher};
pub use license::{Consent, LicenseClass};
pub use misspelling::Misspelling;
pub use store::{DictionaryStore, InstalledMeta};
pub use tokenizer::{tokenize, Word};
