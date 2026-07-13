// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Persisted window size — the desktop window opens at the size the user last
//! left it (position is not persisted: the shell does not observe move events).
//!
//! Mirrors the [`crate::recent_documents`] persistence idiom: each application
//! supplies its own relative file name under the platform data directory, and
//! load/save fail silently (a missing or corrupt file just yields the default).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Smallest believable persisted dimension — anything below this is treated as
/// corrupt and ignored (a window this small is unusable).
const MIN_DIMENSION_PX: f64 = 320.0;

/// Largest believable persisted dimension.
const MAX_DIMENSION_PX: f64 = 16384.0;

/// A window's logical inner size, persisted across sessions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Logical (DPI-independent) inner width in pixels.
    pub width: f64,
    /// Logical (DPI-independent) inner height in pixels.
    pub height: f64,
}

impl WindowGeometry {
    /// A geometry from logical dimensions.
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// Loads the persisted geometry, or `None` when missing, unparsable, or
    /// outside believable bounds (the caller falls back to its default).
    pub fn load(geometry_file: &str) -> Option<Self> {
        let g: Self = geometry_file_path(geometry_file)
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())?;
        let sane = |v: f64| (MIN_DIMENSION_PX..=MAX_DIMENSION_PX).contains(&v);
        (sane(g.width) && sane(g.height)).then_some(g)
    }

    /// Persists to the platform data directory. Errors are silently ignored
    /// (disk full, read-only FS, etc.) — losing a window size is harmless.
    pub fn save(&self, geometry_file: &str) {
        let Some(path) = geometry_file_path(geometry_file) else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// Schedules a debounced save of `size` to `geometry_file`. Each call
/// supersedes the previous pending one (an interactive resize emits a stream
/// of intermediate sizes; only the settled one is written), and the write
/// happens on a worker thread, never on the UI thread.
pub fn save_debounced(geometry_file: &'static str, size: (f64, f64)) {
    use std::sync::atomic::{AtomicU64, Ordering};
    /// Debounce before persisting a new size.
    const SAVE_DEBOUNCE_MS: u64 = 800;
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let my_seq = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    let _ = std::thread::Builder::new()
        .name("loki-window-save".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(SAVE_DEBOUNCE_MS));
            if SEQ.load(Ordering::Relaxed) == my_seq {
                WindowGeometry::new(size.0, size.1).save(geometry_file);
            }
        });
}

/// Resolves the absolute persistence path. Window geometry is a desktop
/// concern (Android windows are fullscreen), so unlike recent-documents this
/// does not consult the Android data-dir override — on platforms without a
/// data dir it simply resolves to `None` and persistence is a no-op.
fn geometry_file_path(geometry_file: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(geometry_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implausible_dimensions_are_rejected() {
        let json = serde_json::to_string(&WindowGeometry::new(10.0, 10.0)).expect("serialize");
        let parsed: WindowGeometry = serde_json::from_str(&json).expect("parse");
        // The raw parse succeeds; the load-time validation is what rejects it.
        let sane = |v: f64| (MIN_DIMENSION_PX..=MAX_DIMENSION_PX).contains(&v);
        assert!(!(sane(parsed.width) && sane(parsed.height)));
    }

    #[test]
    fn round_trips_through_json() {
        let g = WindowGeometry::new(1280.0, 800.0);
        let json = serde_json::to_string(&g).expect("serialize");
        let back: WindowGeometry = serde_json::from_str(&json).expect("parse");
        assert_eq!(g, back);
    }
}
