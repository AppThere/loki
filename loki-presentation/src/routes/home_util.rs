// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tab and file-token helpers for the Home route.
//!
//! Extracted from `home.rs` to keep that file under the 300-line ceiling.

use dioxus::prelude::*;
use loki_file_access::FileAccessToken;

use crate::sessions::DocSessions;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(super) fn push_or_switch_tab(
    mut tabs: Signal<Vec<OpenTab>>,
    mut active_tab: Signal<usize>,
    path: String,
) {
    let title = display_title_from_path(&path);
    let existing = tabs.read().iter().position(|t| t.path == path);
    if let Some(idx) = existing {
        *active_tab.write() = idx + 1;
    } else {
        tabs.write().push(OpenTab {
            title,
            path,
            is_dirty: false,
            is_discarded: false,
        });
        *active_tab.write() = tabs.read().len(); // new tab is last; +1 for Home
    }
}

/// Close any open tab whose `path` matches `path`, resetting the active tab to
/// Home when the closed (or a now-shifted) tab was selected, and drop any
/// stashed editing session for that path.
///
/// The session must be dropped here (not only in the shell's tab-close button):
/// deleting a presentation from the recents list while it is open, or with a
/// session stashed from an earlier tab switch, would otherwise leak the whole
/// `LoroDoc`/deck in the map — and a later file created at the same token key
/// would restore the deleted deck's content instead of loading fresh.
pub(super) fn close_tab_for_path(
    mut tabs: Signal<Vec<OpenTab>>,
    mut active_tab: Signal<usize>,
    mut sessions: Signal<DocSessions>,
    path: &str,
) {
    let removed = tabs.read().iter().position(|t| t.path == path);
    if let Some(idx) = removed {
        tabs.write().remove(idx);
        // active_tab is 1-based (index 0 = Home). Reset to Home if the active
        // selection pointed at or past the removed tab to avoid a stale index.
        if *active_tab.read() > idx {
            *active_tab.write() = 0;
        }
    }
    // Drop the stashed session regardless of whether a tab was open — a session
    // can outlive its tab (stashed on tab switch, then the tab closed).
    sessions.write().remove(path);
}

/// Build a "<stem> Copy.<ext>" filename from a token's display name.
pub(super) fn suggested_copy_name(token: &FileAccessToken) -> String {
    let name = token.display_name();
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} Copy.{ext}"),
        _ => format!("{name} Copy"),
    }
}
