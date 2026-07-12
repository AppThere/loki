// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Persistence for the user's personal spell-check dictionary (5.10).
//!
//! A plain JSON array of lowercased words in the suite data directory,
//! mirroring the `recent_documents` pattern: load-or-empty on any error, and
//! silently-ignored write failures (disk full, read-only FS) so spell checking
//! never crashes the app. The path is threaded through [`super::SpellService`]
//! so tests can point it at a scratch file (or disable persistence with
//! `None`) instead of the real user profile.

use std::path::{Path, PathBuf};

/// The real location: `<data dir>/AppThere/Loki/personal-dictionary.json`
/// (Android-aware via `data_root`). `None` when the platform reports no data
/// directory — persistence is then disabled for the session.
pub(super) fn default_path() -> Option<PathBuf> {
    super::data_root().map(|d| {
        d.join("AppThere")
            .join("Loki")
            .join("personal-dictionary.json")
    })
}

/// Loads the persisted word list; empty on a missing/unreadable/invalid file
/// or when persistence is disabled (`None`).
pub(super) fn load(path: Option<&Path>) -> Vec<String> {
    path.and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Persists `words`, creating parent directories as needed. Errors are
/// silently ignored; a no-op when persistence is disabled (`None`).
pub(super) fn save(path: Option<&Path>, words: &[String]) {
    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(words) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("loki-personal-dict-test-{name}.json"))
    }

    #[test]
    fn round_trips_through_json() {
        let path = scratch_file("roundtrip");
        let words = vec!["apple".to_string(), "zebra".to_string()];
        save(Some(&path), &words);
        assert_eq!(load(Some(&path)), words);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_file_and_disabled_persistence_load_empty() {
        assert!(load(Some(Path::new("/nonexistent/loki/pd.json"))).is_empty());
        assert!(load(None).is_empty());
        save(None, &["ignored".to_string()]); // must not panic
    }

    #[test]
    fn malformed_json_loads_empty() {
        let path = scratch_file("malformed");
        std::fs::write(&path, "{not json").expect("write scratch");
        assert!(load(Some(&path)).is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
