// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Error types for `loki-spell`.

use thiserror::Error;

/// Errors that can occur while loading dictionaries or mutating a checker.
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

    /// The embedded or supplied dictionary catalog could not be parsed.
    #[error("failed to parse dictionary catalog: {0}")]
    CatalogParse(String),

    /// A filesystem operation on the dictionary store failed.
    #[error("dictionary store I/O error: {0}")]
    Io(String),

    /// The requested locale is not installed in the store.
    #[error("no dictionary installed for locale '{0}'")]
    NotInstalled(String),

    /// The catalog entry has no download source (e.g. a bundled-only entry).
    #[error("dictionary '{0}' has no download source")]
    NoSource(String),

    /// Downloading a dictionary file failed in the caller-supplied fetcher.
    #[error("failed to download dictionary: {0}")]
    Download(String),

    /// A downloaded file did not match the catalog's size/SHA-256, so it was
    /// rejected rather than installed.
    #[error("integrity check failed for {tag} {file}: {detail}")]
    Integrity {
        /// The locale tag being installed.
        tag: String,
        /// Which file failed (`aff` or `dic`).
        file: String,
        /// Human-readable mismatch detail.
        detail: String,
    },

    /// A non-permissive (copyleft) dictionary was requested without consent.
    #[error("installing '{tag}' ({license}) requires explicit user consent")]
    ConsentRequired {
        /// The locale tag.
        tag: String,
        /// The SPDX license expression that triggered the gate.
        license: String,
    },
}

/// Convenience alias for `Result<T, SpellError>`.
pub type SpellResult<T> = Result<T, SpellError>;
