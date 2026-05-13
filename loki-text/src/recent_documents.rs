// SPDX-License-Identifier: Apache-2.0

//! Recent-documents list — persisted as JSON in the platform data directory.
//!
//! The list is stored at `{data_dir}/AppThere/Loki/recent.json` and capped at
//! [`MAX_RECENT`] entries.  `untitled://` paths are never recorded.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::new_document;

const MAX_RECENT: usize = 20;
const RECENT_FILE: &str = "AppThere/Loki/recent.json";

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
/// Inject as a `Signal<RecentDocuments>` at the app root so all components
/// can read and mutate the list reactively.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentDocuments {
    pub entries: Vec<RecentEntry>,
}

impl RecentDocuments {
    /// Load from the platform data directory, or return an empty list on any
    /// error (missing file, parse failure, permissions).
    pub fn load() -> Self {
        recent_file_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Persist to the platform data directory.  Errors are silently ignored
    /// (disk full, read-only FS, etc.) — the app continues without crashing.
    pub fn save(&self) {
        let Some(path) = recent_file_path() else {
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
    /// Moves the entry to the front if already present; otherwise inserts at
    /// the front.  Caps the list at [`MAX_RECENT`] entries.
    /// `untitled://` paths are silently ignored.
    pub fn record(&mut self, path: String, title: String) {
        if new_document::is_untitled(&path) {
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

fn recent_file_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(RECENT_FILE))
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
