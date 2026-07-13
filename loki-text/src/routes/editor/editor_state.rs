// SPDX-License-Identifier: Apache-2.0

//! Per-document editor signals and shared state initialisation.
//!
//! [`use_editor_state`] is called once per [`super::editor_inner::EditorInner`]
//! mount.  Because `EditorInner` is keyed on the document path, this is
//! effectively called once per document open, giving each document a clean,
//! isolated set of signals.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use loki_renderer::ViewMode;

use super::editor_scrollbar::{CanvasMounted, ScrollMetrics, ThumbDrag};
use crate::editing::cursor::CursorState;
use crate::editing::saved_state::SavedStateHandle;
use crate::editing::state::DocumentState;
use crate::editing::touch::TouchInteractionState;

// EditorMode removed — the editor is always in edit mode when a document is
// open. Distraction-free reading is handled by the View ribbon tab (future
// pass), not by a separate mode.

/// Draft edit state for the style catalog editor panel.
///
/// `None` on the outer signal → editor closed.  `Some(draft)` → editor open,
/// editing the catalog style identified by `draft.id`.  String fields use an
/// empty string to represent `None` so they bind cleanly to text inputs.
///
/// `font_weight` carries the numeric OpenType weight (1–1000; 400 = Regular,
/// 700 = Bold) the selector edits; bold is derived from it on save. Numeric
/// fields are stored as their string form so they bind directly to text
/// inputs; an empty string represents "inherit / unset".
#[derive(Clone, PartialEq)]
pub(super) struct StyleDraft {
    pub id: String,
    pub name: String,
    pub parent: String,
    pub next: String,
    pub alignment: String,
    pub font_name: String,
    pub font_size_str: String,
    pub font_weight: u16,
    pub italic: bool,
    pub underline: bool,
    pub space_before_str: String,
    pub space_after_str: String,
    pub line_height_str: String,
    pub indent_start_str: String,
    pub indent_end_str: String,
    pub indent_first_str: String,
    pub indent_hanging_str: String,
    pub is_custom: bool,
}

impl Default for StyleDraft {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            parent: String::new(),
            next: String::new(),
            alignment: String::new(),
            font_name: String::new(),
            font_size_str: String::new(),
            font_weight: 400,
            italic: false,
            underline: false,
            space_before_str: String::new(),
            space_after_str: String::new(),
            line_height_str: String::new(),
            indent_start_str: String::new(),
            indent_end_str: String::new(),
            indent_first_str: String::new(),
            indent_hanging_str: String::new(),
            is_custom: false,
        }
    }
}

/// A transient status line (save/export/insert results, style-edit errors).
///
/// Successes surface as an auto-clearing status-bar chip (cleared after a few
/// seconds, or immediately once the document is edited again, so a stale
/// "Document saved" can never sit over a dirty document). Errors keep the
/// persistent banner so failures are not silently hidden.
#[derive(Clone, PartialEq)]
pub(super) struct SaveStatus {
    pub text: String,
    pub is_error: bool,
}

impl SaveStatus {
    /// A success/info status — shown as the auto-clearing status-bar chip.
    pub fn ok(text: String) -> Self {
        Self {
            text,
            is_error: false,
        }
    }

    /// An error status — shown in the persistent banner until dismissed.
    pub fn error(text: String) -> Self {
        Self {
            text,
            is_error: true,
        }
    }
}

/// Which colour the docked colour-picker panel is editing (`None` = closed).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum ColorPickerTarget {
    /// Text (font) colour — hex mark values, custom entry enabled.
    Text,
    /// Highlight — named `HighlightColor` variants only.
    Highlight,
}

/// All per-document signals for the editor, grouped for ergonomic initialisation.
pub(super) struct EditorState {
    pub doc_state: Arc<Mutex<DocumentState>>,
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    pub cursor_state: Signal<CursorState>,
    pub is_dragging: Signal<bool>,
    pub drag_origin: Signal<Option<(f32, f32)>>,
    pub touch_state: Signal<Option<TouchInteractionState>>,
    pub scroll_offset: Signal<f32>,
    /// Live scroll geometry of the canvas container, mirrored from the most
    /// recent DOM `scroll` event and consumed by the custom scrollbars.
    pub scroll_metrics: Signal<ScrollMetrics>,
    /// `MountedData` for the scroll container, captured via `onmounted`; lets the
    /// scrollbar thumbs drive programmatic scrolling (dioxus-native scroll patch).
    pub canvas_mounted: CanvasMounted,
    /// In-progress vertical / horizontal scrollbar thumb drags.
    pub vbar_drag: ThumbDrag,
    pub hbar_drag: ThumbDrag,
    pub current_page: Signal<u32>,
    pub total_pages: Signal<u32>,
    /// Active layout mode (paginated vs reflowable). Defaults by viewport width
    /// (see `editor_inner`) until the user toggles it.
    pub view_mode: Signal<ViewMode>,
    /// `true` once the user has explicitly chosen a view mode, which freezes the
    /// automatic width-based default so it stops overriding their choice.
    pub view_mode_user_set: Signal<bool>,
    /// Active state of inline character formatting at the cursor position.
    /// Updated whenever the cursor moves or a formatting toggle is applied.
    pub bold_active: Signal<bool>,
    pub italic_active: Signal<bool>,
    pub underline_active: Signal<bool>,
    pub strikethrough_active: Signal<bool>,
    pub superscript_active: Signal<bool>,
    pub subscript_active: Signal<bool>,
    /// Loro undo manager — `None` until the document loads.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Clean-checkpoint tracker paired with `undo_manager` (plan 4b.3): save
    /// records the undo-stack depth, so undoing back to the saved state
    /// clears the tab's dirty indicator. Replaced together with the manager
    /// (load, post-save compaction swap) and stashed with it in the tab's
    /// `DocSession`.
    pub saved_state: Signal<SavedStateHandle>,
    /// Whether Ctrl+Z is currently applicable (derived from `undo_manager`).
    pub can_undo: Signal<bool>,
    /// Whether Ctrl+Y / Ctrl+Shift+Z is currently applicable.
    pub can_redo: Signal<bool>,
    /// Whether the style picker panel is currently open above the ribbon.
    pub is_style_picker_open: Signal<bool>,
    /// Style catalog editor draft — `Some` when the editor panel is open.
    pub editing_style_draft: Signal<Option<StyleDraft>>,
    /// Paginated render zoom, in percent (100 = 1:1). Cycled by the status
    /// bar's zoom badge; scales page tiles + paint together (4c.5 / F6d).
    pub zoom_percent: Signal<u32>,
    /// Whether the document has unsaved changes (mirror of the tab's dirty
    /// indicator; plan 4b.3). Drives the ribbon Save button's disabled state —
    /// a clean *titled* document has nothing to save. Untitled documents stay
    /// dirty (Save routes to Save As), so their Save button stays enabled.
    pub is_dirty: Signal<bool>,
    /// Last transient status (`None` = nothing to show). See [`SaveStatus`]
    /// for how successes vs errors are surfaced.
    pub save_message: Signal<Option<SaveStatus>>,
    /// Monotonic counter bumped by the Ctrl+S handler. `EditorInner` watches it
    /// and runs the save (or Save As for untitled documents) — the keydown
    /// handler has no access to the tab/recents context, so it signals instead.
    pub save_request: Signal<u32>,
    /// Index of the active ribbon tab (0 = Write, 1 = Format, … Publish is
    /// the last core tab; the Table contextual tab follows).
    pub active_ribbon_tab: Signal<usize>,
    /// Which colour the docked picker panel is editing (`None` = closed).
    pub open_color_picker: Signal<Option<ColorPickerTarget>>,
    /// Recent Format-tab font-colour picks (hex, most recent first).
    pub recent_text_colors: Signal<Vec<String>>,
    /// Recent Format-tab highlight picks (variant names, most recent first).
    pub recent_highlights: Signal<Vec<String>>,
    /// Whether the Publish-tab PDF/X export panel is open above the ribbon.
    pub is_publish_panel_open: Signal<bool>,
    /// Selected PDF/X conformance level for export.
    pub pdf_level: Signal<super::editor_publish::PdfXLevelChoice>,
    /// Metadata editor draft — `Some` when the Dublin Core panel is open.
    pub editing_metadata: Signal<Option<super::editor_metadata::MetaDraft>>,
}

/// Initialises and returns all per-document editing signals.
///
/// Acts as a Dioxus custom hook — must be called unconditionally at the top
/// of `EditorInner`.  Hook call order is preserved because `EditorInner`
/// always calls this as its first hook operation.
pub(super) fn use_editor_state() -> EditorState {
    // Share the app-wide font handle (warm-up started at launch in `App`) so
    // mounting the editor never pays — or waits for — the system-font scan.
    // The fallback covers tests/hosts that mount the editor without `App`.
    let doc_state: Arc<Mutex<DocumentState>> = use_hook(|| {
        let fonts = try_consume_context::<loki_layout::SharedFontResources>()
            .unwrap_or_else(loki_layout::SharedFontResources::warm_up);
        Arc::new(Mutex::new(DocumentState::with_fonts(fonts)))
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
        scroll_offset: use_signal(|| 0.0_f32),
        scroll_metrics: use_signal(ScrollMetrics::default),
        canvas_mounted: use_signal(|| None),
        vbar_drag: use_signal(|| None),
        hbar_drag: use_signal(|| None),
        current_page,
        total_pages,
        view_mode: use_signal(ViewMode::default),
        view_mode_user_set: use_signal(|| false),
        bold_active: use_signal(|| false),
        italic_active: use_signal(|| false),
        underline_active: use_signal(|| false),
        strikethrough_active: use_signal(|| false),
        superscript_active: use_signal(|| false),
        subscript_active: use_signal(|| false),
        undo_manager: use_signal(|| None),
        saved_state: use_signal(SavedStateHandle::new),
        can_undo: use_signal(|| false),
        can_redo: use_signal(|| false),
        is_style_picker_open: use_signal(|| false),
        editing_style_draft: use_signal(|| None),
        zoom_percent: use_signal(|| 100_u32),
        is_dirty: use_signal(|| false),
        save_message: use_signal(|| None),
        save_request: use_signal(|| 0_u32),
        active_ribbon_tab: use_signal(|| 0_usize),
        open_color_picker: use_signal(|| None),
        recent_text_colors: use_signal(Vec::new),
        recent_highlights: use_signal(Vec::new),
        is_publish_panel_open: use_signal(|| false),
        pdf_level: use_signal(super::editor_publish::PdfXLevelChoice::default),
        editing_metadata: use_signal(|| None),
    }
}
