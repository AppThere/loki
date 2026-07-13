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

/// Push `path` as a new open tab, or switch to its existing tab if already open.
///
/// Thin `Signal` wrapper over the shared [`loki_app_shell::tabs::open_or_switch`]
/// logic (deduplicated across the three app shells — plan 7.2).
pub(super) fn push_or_switch_tab(
    mut tabs: Signal<Vec<OpenTab>>,
    mut active_tab: Signal<usize>,
    path: String,
) {
    let title = display_title_from_path(&path);
    *active_tab.write() = loki_app_shell::tabs::open_or_switch(&mut tabs.write(), path, title);
}

/// Close any open tab whose `path` matches `path`, resetting the active tab to
/// Home when the closed (or a now-shifted) tab was selected, and drop any
/// stashed editing session for that path.
///
/// The session must be dropped here (not only in the shell's tab-close button):
/// deleting a workbook from the recents list while it is open, or with a session
/// stashed from an earlier tab switch, would otherwise leak the whole
/// `LoroDoc`/workbook in the map — and a later file created at the same token
/// key would restore the deleted workbook's content instead of loading fresh.
/// Tab bookkeeping is the shared [`loki_app_shell::tabs::close_by_path`] logic.
pub(super) fn close_tab_for_path(
    mut tabs: Signal<Vec<OpenTab>>,
    mut active_tab: Signal<usize>,
    mut sessions: Signal<DocSessions>,
    path: &str,
) {
    let active = *active_tab.read();
    let new_active = loki_app_shell::tabs::close_by_path(&mut tabs.write(), active, path);
    if new_active != active {
        *active_tab.write() = new_active;
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
