// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared per-user application-data directory resolution for the Loki suite.
//!
//! All persisted app state (the spell-checker dictionary cache, the macro trust
//! store, …) hangs off one platform data directory so the three binaries agree
//! on where user data lives. Kept in one place so the path convention has a
//! single definition.

use std::path::PathBuf;

/// The platform per-user data directory (e.g. `~/.local/share` on Linux,
/// `%APPDATA%` on Windows, the app sandbox on Android), or `None` when the
/// platform reports none.
#[must_use]
pub fn data_root() -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    {
        crate::recent_documents::android_data_dir()
    }
    #[cfg(not(target_os = "android"))]
    {
        dirs::data_dir()
    }
}

/// The suite data directory root (`<data>/AppThere/Loki`).
#[must_use]
pub fn suite_dir() -> Option<PathBuf> {
    data_root().map(|d| d.join("AppThere").join("Loki"))
}

/// The macro trust-store file (`<suite>/macro-trust.json`), keyed by macro
/// payload hash (macro spec §2.4). `None` when no data directory is available
/// (the trust store then runs in-memory).
#[must_use]
pub fn macro_trust_store_path() -> Option<PathBuf> {
    suite_dir().map(|d| d.join("macro-trust.json"))
}
