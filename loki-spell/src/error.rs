// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Error types for `loki-spell`.

use thiserror::Error;

/// Errors that can occur while loading or mutating a spell-checking dictionary.
///
/// The underlying `spellbook` crate is `no_std` and does not implement
/// [`std::error::Error`] on its parse errors, so their messages are captured as
/// owned strings at this boundary rather than wrapped via `#[from]`.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum SpellError {
    /// The Hunspell `.aff`/`.dic` pair could not be parsed.
    #[error("failed to parse dictionary: {0}")]
    DictionaryParse(String),

    /// A word could not be added to the in-memory dictionary (invalid flags).
    #[error("failed to add word to dictionary: {0}")]
    WordAdd(String),
}

/// Convenience alias for `Result<T, SpellError>`.
pub type SpellResult<T> = Result<T, SpellError>;
