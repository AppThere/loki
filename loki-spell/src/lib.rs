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
//! 1. Load a [`SpellChecker`] per language from Hunspell `.aff` + `.dic` data
//!    (such dictionaries ship with LibreOffice / Mozilla for every locale).
//! 2. Call [`SpellChecker::check_text`] on a run of text (e.g. a paragraph) to
//!    get a [`Misspelling`] per flagged word, each carrying the word's byte
//!    range so the renderer can position a squiggle decoration.
//! 3. Offer [`SpellChecker::suggest`] corrections in a context menu, and let the
//!    user [`SpellChecker::add_word`] (personal dictionary) or
//!    [`SpellChecker::ignore_word`] (this session).
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

pub mod checker;
pub mod error;
pub mod misspelling;
pub mod tokenizer;

pub use checker::SpellChecker;
pub use error::{SpellError, SpellResult};
pub use misspelling::Misspelling;
pub use tokenizer::{tokenize, Word};
