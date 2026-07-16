// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Typed errors for the macro host (trust store I/O and capability decisions).

use std::path::PathBuf;

/// Errors raised by the macro host.
///
/// Trust-store persistence is best-effort by design (a missing or corrupt store
/// simply means "no documents are trusted yet"), so most of these surface only
/// when an explicit save/load is requested; a broken store never blocks opening
/// a document.
#[derive(Debug, thiserror::Error)]
pub enum MacroHostError {
    /// The trust store could not be read from or written to disk.
    #[error("trust store I/O error at {path}: {source}")]
    Io {
        /// The path that was being read or written.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The on-disk trust store was present but could not be parsed. The store
    /// is treated as empty so a corrupt file never blocks document opening; the
    /// error is surfaced for logging/diagnostics only.
    #[error("trust store at {path} is corrupt: {reason}")]
    Corrupt {
        /// The path of the unreadable store.
        path: PathBuf,
        /// A human-readable parse reason.
        reason: String,
    },
}
