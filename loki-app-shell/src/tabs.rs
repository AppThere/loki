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
