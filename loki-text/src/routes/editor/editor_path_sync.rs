// SPDX-License-Identifier: Apache-2.0

//! Path-change detection and per-document state reset for
//! [`super::editor_inner::EditorInner`].

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// Synchronises `path_signal` with the `path` prop and resets all per-document
/// signals when the active document changes.
#[allow(clippy::too_many_arguments)] // 12 args: all per-document reset targets
pub(super) fn sync_path_and_reset(
    path: &str,
    path_signal: &mut Signal<String>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: &mut Signal<CursorState>,
    loro_doc: &mut Signal<Option<loro::LoroDoc>>,
    undo_manager: &mut Signal<Option<loro::UndoManager>>,
    total_pages: &mut Signal<u32>,
    current_page: &mut Signal<u32>,
    can_undo: &mut Signal<bool>,
    can_redo: &mut Signal<bool>,
    dismiss_font_warning: &mut Signal<bool>,
    is_style_picker_open: &mut Signal<bool>,
) {
    let current = path_signal.peek().clone();
    if current == path {
        return;
    }
    tracing::debug!(
        "EditorInner: path changed from {} to {} → resetting per-document state",
        current,
        path
    );
    path_signal.set(path.to_owned());

    if let Ok(mut state) = doc_state.lock() {
        state.document = None;
        state.generation = 0;
        state.page_count = 0;
        state.paginated_layout = None;
    } else {
        tracing::error!("doc_state lock poisoned during tab switch — state may be stale");
    }

    cursor_state.set(CursorState::default());
    loro_doc.set(None);
    undo_manager.set(None);
    total_pages.set(0);
    current_page.set(1);
    can_undo.set(false);
    can_redo.set(false);
    dismiss_font_warning.set(false);
    is_style_picker_open.set(false);
}
