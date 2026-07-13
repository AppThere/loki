// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Open-tab state shared across the Loki editor shells.

/// Represents a single open document tab.
///
/// Injected into Dioxus context at each application's `App` root as a
/// `Signal<Vec<OpenTab>>`, alongside a `Signal<usize>` for the active tab index
/// (`0` = the Home tab).
#[derive(Clone, PartialEq)]
pub struct OpenTab {
    /// Display title shown in the tab bar (filename stem, decoded).
    pub title: String,
    /// The serialised file access token / path used by the editor.
    pub path: String,
    /// Whether the document has unsaved changes.
    pub is_dirty: bool,
    /// Whether this tab has been discarded from memory.
    pub is_discarded: bool,
}

// ── Shared tab-list operations ────────────────────────────────────────────────
//
// These are pure functions over the tab vector and the 1-based active index
// (`0` = Home), so they carry no Dioxus dependency and are unit-tested here
// once. Each shell keeps a thin `Signal` wrapper that reads/writes its own
// signals and drops the app-specific editing session; the *logic* — which was
// copied into all three `routes/` modules — lives here.

/// Open `path` as a new tab, or switch to its existing tab if already open.
/// Returns the new 1-based active-tab index (`0` = Home).
#[must_use]
pub fn open_or_switch(tabs: &mut Vec<OpenTab>, path: String, title: String) -> usize {
    if let Some(idx) = tabs.iter().position(|t| t.path == path) {
        idx + 1
    } else {
        tabs.push(OpenTab {
            title,
            path,
            is_dirty: false,
            is_discarded: false,
        });
        tabs.len() // new tab is last; +1 for the Home tab.
    }
}

/// Close any open tab whose `path` matches, returning the new 1-based active
/// index: unchanged, or reset to `0` (Home) when the active selection pointed
/// at or past the removed tab (avoiding a stale index). The caller drops the
/// stashed editing session for `path` regardless of the return — a session can
/// outlive its tab (stashed on tab switch, then the tab closed).
#[must_use]
pub fn close_by_path(tabs: &mut Vec<OpenTab>, active: usize, path: &str) -> usize {
    if let Some(idx) = tabs.iter().position(|t| t.path == path) {
        tabs.remove(idx);
        // `active` is 1-based (0 = Home), `idx` is 0-based.
        if active > idx {
            return 0;
        }
    }
    active
}

/// Where to navigate after closing a tab from the tab bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabCloseNav {
    /// Active tab and route unchanged.
    Stay,
    /// Navigate to the Home route.
    Home,
    /// Navigate to the editor for the tab now at this 0-based vector index.
    Editor(usize),
}

/// The result of resolving a tab-bar close.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabCloseOutcome {
    /// The new 1-based active-tab index (`0` = Home).
    pub new_active: usize,
    /// Where to navigate.
    pub nav: TabCloseNav,
}

/// Resolve closing the tab at 1-based tab-bar `idx` (`0` = Home, which cannot be
/// closed), given the currently active index and the tab count *before*
/// removal.
///
/// Returns `None` when the close is a no-op — the Home tab, or a stale index
/// past the end (a rapid second close, or a close confirmed after the list
/// changed). Otherwise the caller removes the tab at `idx - 1`, drops its
/// session, sets `new_active`, and applies `nav` (looking up the path at the
/// [`TabCloseNav::Editor`] vector index in the *post-removal* list).
#[must_use]
pub fn resolve_tab_close(
    idx: usize,
    current_active: usize,
    len_before: usize,
) -> Option<TabCloseOutcome> {
    if idx == 0 {
        return None; // Home tab cannot be closed.
    }
    let vec_idx = idx - 1;
    if vec_idx >= len_before {
        return None; // stale index: idx captured before the list shrank.
    }

    if len_before - 1 == 0 {
        // Closed the last document tab → fall back to Home.
        return Some(TabCloseOutcome {
            new_active: 0,
            nav: TabCloseNav::Home,
        });
    }
    if idx == current_active {
        // Closed the active tab → activate its left neighbour (or the first
        // remaining tab when the first was closed).
        let new_active = if vec_idx > 0 { idx - 1 } else { 1 };
        return Some(TabCloseOutcome {
            new_active,
            nav: TabCloseNav::Editor(new_active - 1),
        });
    }
    if idx < current_active {
        // Closed a tab left of the active one → the active shifts left by one.
        return Some(TabCloseOutcome {
            new_active: current_active - 1,
            nav: TabCloseNav::Stay,
        });
    }
    // Closed a tab right of the active one → active index unchanged.
    Some(TabCloseOutcome {
        new_active: current_active,
        nav: TabCloseNav::Stay,
    })
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
