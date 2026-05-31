// SPDX-License-Identifier: Apache-2.0

//! Per-document editor signals and shared state initialisation.
//!
//! [`use_editor_state`] is called once per [`super::editor_inner::EditorInner`]
//! mount.  Because `EditorInner` is keyed on the document path, this is
//! effectively called once per document open, giving each document a clean,
//! isolated set of signals.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;
use crate::editing::touch::TouchInteractionState;

// EditorMode removed — the editor is always in edit mode when a document is
// open. Distraction-free reading is handled by the View ribbon tab (future
// pass), not by a separate mode.

/// All per-document signals for the editor, grouped for ergonomic initialisation.
pub(super) struct EditorState {
    pub doc_state: Arc<Mutex<DocumentState>>,
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    pub cursor_state: Signal<CursorState>,
    pub is_dragging: Signal<bool>,
    pub drag_origin: Signal<Option<(f32, f32)>>,
    pub touch_state: Signal<Option<TouchInteractionState>>,
    pub window_width: Signal<f32>,
    pub scroll_offset: Signal<f32>,
    pub current_page: Signal<u32>,
    pub total_pages: Signal<u32>,
    /// Active state of inline character formatting at the cursor position.
    /// Updated whenever the cursor moves or a formatting toggle is applied.
    pub bold_active: Signal<bool>,
    pub italic_active: Signal<bool>,
    pub underline_active: Signal<bool>,
    pub strikethrough_active: Signal<bool>,
    pub superscript_active: Signal<bool>,
    pub subscript_active: Signal<bool>,
    /// Loro undo manager — `None` until the document loads.
    ///
    /// // TODO(undo-dirty): `can_undo` does not track whether the document is
    /// saved relative to the undo stack.  When a Save action is implemented,
    /// call `UndoManager::record_new_checkpoint()` to mark the clean state so
    /// the ribbon Save button can be disabled when there is nothing to save.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether Ctrl+Z is currently applicable (derived from `undo_manager`).
    pub can_undo: Signal<bool>,
    /// Whether Ctrl+Y / Ctrl+Shift+Z is currently applicable.
    pub can_redo: Signal<bool>,
    /// Whether the style picker panel is currently open above the ribbon.
    pub is_style_picker_open: Signal<bool>,
}

/// Initialises and returns all per-document editing signals.
///
/// Acts as a Dioxus custom hook — must be called unconditionally at the top
/// of `EditorInner`.  Hook call order is preserved because `EditorInner`
/// always calls this as its first hook operation.
pub(super) fn use_editor_state() -> EditorState {
    let doc_state: Arc<Mutex<DocumentState>> =
        use_hook(|| Arc::new(Mutex::new(DocumentState::new())));

    // Synchronously read page_count in case doc_state is already populated
    // (covers tab-switch-back where the document was previously loaded).
    // In practice doc_state is always fresh (use_hook creates it), but this
    // guard is harmless and future-proofs against in-place reuse.
    //
    // COMPAT(dioxus): signal.set() called during the render phase to
    // synchronously initialise from pre-existing state.
    let current_page = use_signal(|| 1_u32);
    let total_pages = {
        let initial = doc_state.lock().map(|s| s.page_count as u32).unwrap_or(0);
        use_signal(|| initial)
    };

    EditorState {
        doc_state,
        loro_doc: use_signal(|| None),
        cursor_state: use_signal(CursorState::new),
        is_dragging: use_signal(|| false),
        drag_origin: use_signal(|| None),
        touch_state: use_signal(|| None),
        window_width: use_signal(|| 1280.0_f32),
        scroll_offset: use_signal(|| 0.0_f32),
        current_page,
        total_pages,
        bold_active: use_signal(|| false),
        italic_active: use_signal(|| false),
        underline_active: use_signal(|| false),
        strikethrough_active: use_signal(|| false),
        superscript_active: use_signal(|| false),
        subscript_active: use_signal(|| false),
        undo_manager: use_signal(|| None),
        can_undo: use_signal(|| false),
        can_redo: use_signal(|| false),
        is_style_picker_open: use_signal(|| false),
    }
}
