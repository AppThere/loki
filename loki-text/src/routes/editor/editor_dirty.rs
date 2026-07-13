// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unsaved-changes (dirty) tracking effect for the document editor.
//!
//! Extracted from `editor_inner.rs` to keep that file under the 300-line
//! ceiling.

use dioxus::prelude::*;

use super::editor_state::SaveStatus;
use crate::editing::cursor::CursorState;
use crate::editing::saved_state::SavedStateHandle;
use crate::new_document::is_untitled;
use crate::tabs::OpenTab;

/// Wires the effect that keeps the `is_dirty` signal and the active tab's dirty
/// indicator in sync with the document's edit state.
///
/// Dirty = the live generation differs from the clean baseline AND the
/// undo-stack clean checkpoint disagrees (undoing to the save point clears
/// dirty; plan 4b.3); untitled docs are always dirty until the first Save As.
///
/// Also clears a lingering *success* status chip the moment the document goes
/// dirty — a stale "Document saved" must never sit over unsaved edits.
pub(super) fn use_dirty_tracking(
    cursor_state: Signal<CursorState>,
    path_signal: Signal<String>,
    baseline_gen: Signal<u64>,
    saved_state: Signal<SavedStateHandle>,
    mut is_dirty: Signal<bool>,
    mut tabs: Signal<Vec<OpenTab>>,
    mut save_message: Signal<Option<SaveStatus>>,
) {
    use_effect(move || {
        let live_gen = cursor_state.read().document_generation;
        let path = path_signal();
        let base = baseline_gen();
        let undo_clean = saved_state.read().is_clean();
        let dirty = is_untitled(&path) || (live_gen != base && !undo_clean);
        if *is_dirty.peek() != dirty {
            is_dirty.set(dirty); // guard avoids a needless ribbon re-render
        }
        if dirty
            && save_message
                .peek()
                .as_ref()
                .is_some_and(|status| !status.is_error)
        {
            save_message.set(None);
        }
        // Only take a write guard when a tab's flag actually changes. A bare
        // `tabs.write()` marks the shared tabs signal dirty and re-renders the
        // whole tab bar on every keystroke, even when nothing changed — peek
        // first and write only on a real transition.
        let needs_update = tabs
            .peek()
            .iter()
            .any(|tab| tab.path == path && tab.is_dirty != dirty);
        if needs_update {
            let mut t = tabs.write();
            if let Some(tab) = t.iter_mut().find(|tab| tab.path == path) {
                tab.is_dirty = dirty;
            }
        }
    });
}
