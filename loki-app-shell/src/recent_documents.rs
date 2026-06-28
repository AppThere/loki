// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Recent-documents list — persisted as JSON in the platform data directory.
//!
//! Each application shell supplies its own relative file name (e.g.
//! `AppThere/Loki/recent.json`) to [`RecentDocuments::load`]; the name is
//! remembered on the value so [`RecentDocuments::save`] takes no argument. The
//! list is capped at [`MAX_RECENT`] entries and `untitled-N` paths
//! (see [`crate::untitled`]) are never recorded.

use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::untitled::is_untitled;

// ── Android data-dir override ─────────────────────────────────────────────────
// On Android, dirs::data_dir() returns None. android_main() calls
// set_android_data_dir() with the value from AndroidApp::internal_data_path()
// before Dioxus launches, giving recent_file_path() a writable location.

#[cfg(target_os = "android")]
static ANDROID_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Store the Android internal data path before `dioxus::launch`.
/// Safe to call multiple times (ignored after the first call).
#[cfg(target_os = "android")]
pub fn set_android_data_dir(path: PathBuf) {
    let _ = ANDROID_DATA_DIR.set(path);
}

/// Returns the Android internal data path set via [`set_android_data_dir`], if
/// any. Shared with the spell module so dictionaries cache to the same root.
#[cfg(target_os = "android")]
pub(crate) fn android_data_dir() -> Option<PathBuf> {
    ANDROID_DATA_DIR.get().cloned()
}

/// Maximum number of entries retained in the recent-documents list.
const MAX_RECENT: usize = 20;

/// A single entry in the recent-documents list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentEntry {
    /// The serialised file-access token used as the editor route path.
    pub path: String,
    /// Human-readable document title (filename stem).
    pub title: String,
    /// ISO 8601 date of the last open, e.g. `"2026-05-13"`.
    pub modified_at: String,
}

/// In-memory recent-documents list.
///
/// Inject as a `Signal<RecentDocuments>` at the app root so all components can
/// read and mutate the list reactively. Construct with [`RecentDocuments::load`]
/// (which records the persistence file name); the name is not serialised.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentDocuments {
    /// The recent entries, most-recent first.
    pub entries: Vec<RecentEntry>,
    /// Relative persistence path under the platform data dir, e.g.
    /// `"AppThere/Loki/recent.json"`. Set by [`load`](RecentDocuments::load);
    /// skipped during (de)serialisation so the on-disk format is just
    /// `{ "entries": [...] }`.
    #[serde(skip)]
    recent_file: &'static str,
}

impl RecentDocuments {
    /// Load from the platform data directory, or return an empty list on any
    /// error (missing file, parse failure, permissions).
    ///
    /// `recent_file` is the relative path under the platform data dir; it is
    /// stored on the returned value so [`save`](Self::save) needs no argument.
    pub fn load(recent_file: &'static str) -> Self {
        let mut docs: Self = recent_file_path(recent_file)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        docs.recent_file = recent_file;
        docs
    }

    /// Persist to the platform data directory. Errors are silently ignored
    /// (disk full, read-only FS, etc.) — the app continues without crashing.
    /// A no-op if the value was not produced by [`load`](Self::load).
    pub fn save(&self) {
        let Some(path) = recent_file_path(self.recent_file) else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Record a document open.
    ///
    /// Moves the entry to the front if already present; otherwise inserts at the
    /// front. Caps the list at [`MAX_RECENT`] entries. `untitled-N` paths are
    /// silently ignored.
    pub fn record(&mut self, path: String, title: String) {
        if is_untitled(&path) {
            return;
        }
        self.entries.retain(|e| e.path != path);
        self.entries.insert(
            0,
            RecentEntry {
                path,
                title,
                modified_at: today_iso(),
            },
        );
        self.entries.truncate(MAX_RECENT);
    }

    /// Remove a recent entry by path (e.g. after a "remove from list" action).
    pub fn remove(&mut self, path: &str) {
        self.entries.retain(|e| e.path != path);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// On Android CPU the cfg-gated return is always taken; the desktop fallback is
// unreachable on that target but reachable on desktop/GPU.
#[allow(unreachable_code)]
fn recent_file_path(recent_file: &str) -> Option<PathBuf> {
    #[cfg(target_os = "android")]
    return ANDROID_DATA_DIR.get().map(|d| d.join(recent_file));

    dirs::data_dir().map(|d| d.join(recent_file))
}

/// Returns today's date as `"YYYY-MM-DD"` using only `std`.
fn today_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, m, d) = days_to_ymd(secs / 86400);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Civil-calendar conversion from days-since-Unix-epoch to (year, month, day).
///
/// Algorithm: Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms"
/// <https://howardhinnant.github.io/date_algorithms.html>
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> RecentEntry {
        RecentEntry {
            path: path.to_string(),
            title: path.to_string(),
            modified_at: String::new(),
        }
    }

    #[test]
    fn record_inserts_at_front() {
        let mut docs = RecentDocuments::default();
        docs.record("a".into(), "A".into());
        docs.record("b".into(), "B".into());
        let paths: Vec<&str> = docs.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["b", "a"], "most-recent first");
    }

    #[test]
    fn record_deduplicates_and_promotes() {
        let mut docs = RecentDocuments::default();
        docs.record("a".into(), "A".into());
        docs.record("b".into(), "B".into());
        docs.record("a".into(), "A".into()); // re-open a
        let paths: Vec<&str> = docs.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a", "b"], "re-opening promotes without duplicating");
    }

    #[test]
    fn record_ignores_untitled_paths() {
        let mut docs = RecentDocuments::default();
        docs.record("untitled-1".into(), "Untitled".into());
        assert!(docs.entries.is_empty(), "untitled paths are never recorded");
    }

    #[test]
    fn record_caps_at_max_recent() {
        let mut docs = RecentDocuments::default();
        for i in 0..(MAX_RECENT + 5) {
            docs.record(format!("doc-{i}"), format!("Doc {i}"));
        }
        assert_eq!(docs.entries.len(), MAX_RECENT);
        // The newest entry is at the front; the oldest were dropped.
        assert_eq!(docs.entries[0].path, format!("doc-{}", MAX_RECENT + 4));
    }

    #[test]
    fn remove_drops_matching_entry() {
        let mut docs = RecentDocuments {
            entries: vec![entry("a"), entry("b"), entry("c")],
            ..Default::default()
        };
        docs.remove("b");
        let paths: Vec<&str> = docs.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a", "c"]);
    }

    #[test]
    fn entries_round_trip_through_json_without_the_file_name() {
        let mut docs = RecentDocuments::load("AppThere/Loki/recent.json");
        docs.entries = vec![entry("a")];
        let json = serde_json::to_string(&docs).unwrap();
        assert!(
            !json.contains("recent_file"),
            "the persistence file name must not be serialised"
        );
        let back: RecentDocuments = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entries, docs.entries);
    }
}
