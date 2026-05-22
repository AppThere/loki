// SPDX-License-Identifier: Apache-2.0

//! Open-tab state for the `loki-spreadsheet` editor shell.
//!
//! [`OpenTab`] is injected into Dioxus context at the [`crate::app::App`] root
//! as a `Signal<Vec<OpenTab>>`, alongside a `Signal<usize>` for the active
//! tab index (0 = Home tab).

/// Represents a single open document tab.
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
