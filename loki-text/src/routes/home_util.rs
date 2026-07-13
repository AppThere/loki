// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tab and file-token helpers for the Home route.
//!
//! Extracted from `home.rs` to keep that file under the 300-line ceiling.

use dioxus::prelude::*;
use loki_file_access::FileAccessToken;
use loki_i18n::fl;

use crate::sessions::DocSessions;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Push `path` as a new open tab, or switch to its existing tab if already open.
///
/// Thin `Signal` wrapper over the shared [`loki_app_shell::tabs::open_or_switch`]
/// logic (deduplicated across the three app shells — plan 7.2).
// TODO(tabs): Replace router-driven navigation with tab-driven navigation.
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
/// deleting a file from the recents list while it is open, or with a session
/// stashed from an earlier tab switch, would otherwise leak the whole
/// `LoroDoc`/layout in the map — and a later file created at the same token
/// key would restore the deleted document's content instead of loading fresh.
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

/// True if `name` has a template extension (Word `.dotx`/`.dotm` or
/// LibreOffice `.ott`/`.ots`). Templates open as fresh, detached documents.
pub(super) fn is_template_name(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| matches!(e.as_str(), "dotx" | "dotm" | "ott" | "ots"))
}

/// Push `tab` as a new open tab (last position) and return its path so the
/// caller can navigate to the editor.
pub(super) fn push_new_tab(
    mut tabs: Signal<Vec<OpenTab>>,
    mut active_tab: Signal<usize>,
    tab: OpenTab,
) -> String {
    let path = tab.path.clone();
    tabs.write().push(tab);
    *active_tab.write() = tabs.read().len(); // new tab is last; +1 for Home
    path
}

/// Build a "<stem> Copy.<ext>" filename from a token's display name.
pub(super) fn suggested_copy_name(token: &FileAccessToken) -> String {
    let name = token.display_name();
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem} Copy.{ext}"),
        _ => format!("{name} Copy"),
    }
}

// ── Template browser (4c.3) ───────────────────────────────────────────────────

/// The template names shown by the browse dialog, in the exact order
/// `on_template_select` dispatches (0 = Blank, 1..=5 = bundled templates).
pub(super) fn template_names() -> Vec<String> {
    vec![
        fl!("home-template-blank"),
        fl!("home-template-markdown"),
        fl!("home-template-apa"),
        fl!("home-template-mla"),
        fl!("home-template-screenplay"),
        fl!("home-template-resume"),
    ]
}

/// Boundary mount for the template-browser overlay (ADR-0013): owns nothing
/// but the conditional mount, so `home.rs` stays a single line at the call
/// site. Selecting an entry closes the dialog and forwards the index to the
/// same handler the gallery cards use.
#[component]
pub(super) fn TemplateBrowserHost(
    browsing: Signal<bool>,
    on_select: EventHandler<usize>,
) -> Element {
    rsx! {
        {browsing().then(|| rsx! {
            appthere_ui::AtTemplateBrowser {
                title: fl!("home-browse-dialog-title"),
                cancel_label: fl!("home-browse-dialog-cancel"),
                entries: template_names(),
                on_select: move |idx: usize| {
                    browsing.set(false);
                    on_select.call(idx);
                },
                on_cancel: move |_| browsing.set(false),
            }
        })}
    }
}
