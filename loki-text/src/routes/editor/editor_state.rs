// SPDX-License-Identifier: Apache-2.0

//! Per-document editor signals and shared state initialisation.
//!
//! [`use_editor_state`] is called once per [`super::editor_inner::EditorInner`]
//! mount.  Because `EditorInner` is keyed on the document path, this is
//! effectively called once per document open, giving each document a clean,
//! isolated set of signals.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;

use crate::components::document_source::DocumentState;
use crate::editing::cursor::CursorState;
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
}

/// Initialises and returns all per-document editing signals.
///
/// Acts as a Dioxus custom hook — must be called unconditionally at the top
/// of `EditorInner`.  Hook call order is preserved because `EditorInner`
/// always calls this as its first hook operation.
pub(super) fn use_editor_state() -> EditorState {
    let doc_state: Arc<Mutex<DocumentState>> = use_hook(|| {
        Arc::new(Mutex::new(DocumentState {
            document: None,
            generation: 0,
            page_count: 0,
            canvas_width: 0.0,
            visible_rect: None,
            page_width_px: tokens::PAGE_WIDTH_PX,
            page_height_px: tokens::PAGE_HEIGHT_PX,
            cursor_state: None,
            paginated_layout: None,
            preserve_for_editing: false,
            shared_renderer: Arc::new(Mutex::new(None)),
            shared_font_cache: Arc::new(Mutex::new(loki_vello::FontDataCache::new())),
            layout_stamp: 0,
            layout_generation: 0,
            layout_canvas_width: 0.0,
            layout_preserve_for_editing: false,
            shared_font_resources: Arc::new(Mutex::new(loki_layout::FontResources::new())),
        }))
    });

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
    }
}
