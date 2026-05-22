// SPDX-License-Identifier: Apache-2.0

//! Open-tab state for the `loki-presentation` editor shell.

/// Represents a single open document tab.
#[derive(Clone, PartialEq)]
pub struct OpenTab {
    /// Display title shown in the tab bar.
    pub title: String,
    /// The serialised file access token / path used by the editor.
    pub path: String,
    /// Whether the document has unsaved changes.
    pub is_dirty: bool,
    /// Whether this tab has been discarded from memory.
    pub is_discarded: bool,
}
