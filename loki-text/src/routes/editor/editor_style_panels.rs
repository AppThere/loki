// SPDX-License-Identifier: Apache-2.0

//! The two in-flow style surfaces that sit between the canvas and the ribbon:
//! the paragraph style picker and the style catalog editor.
//!
//! # In flow, not overlaid
//!
//! Both are rendered **in the flex column** rather than as overlays. Block-level
//! `position: absolute` is confirmed working in this Blitz build, but these
//! panels displace the canvas rather than covering it — a deliberate layout
//! choice, not a workaround (see `editor_style.rs` for the rationale). Their
//! position in the column is therefore load-bearing, which is why this function
//! returns both in one fragment and `editor_inner` calls it in the exact slot
//! they occupied.
//!
//! Extracted from `editor_inner` (CLAUDE.md technique 3): that file is baselined
//! over the 300-line ceiling and may not grow, and these two mounts are a
//! cohesive cluster with eighteen arguments between them.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use appthere_ui::responsive::Breakpoint;
use dioxus::prelude::*;

use super::editor_state::StyleDraft;
use super::editor_style::style_picker_panel;
use super::editor_style_editor::{StyleEditorSync, style_editor_panel};
use crate::editing::state::DocumentState;

/// The signals the style picker and catalog editor select and edit through.
///
/// Grouped for the same reason [`StyleEditorSync`] is: the two panels need
/// eleven selection handles between them, and a plain argument list that long
/// is one transposed pair away from a silent bug.
#[derive(Clone, Copy)]
pub(super) struct StylePanelState {
    /// Whether the paragraph style picker is open.
    pub is_style_picker_open: Signal<bool>,
    /// Whether the character style picker (Format tab) is open.
    pub is_char_style_picker_open: Signal<bool>,
    /// The picker's filter text.
    pub style_search_query: Signal<String>,
    /// Catalog editor draft — `Some` when the editor panel is open.
    pub editing_style_draft: Signal<Option<StyleDraft>>,
    /// Selected character style in the family browser.
    pub editing_char_style: Signal<Option<String>>,
    /// Editable character-style draft.
    pub editing_char_draft: Signal<Option<StyleDraft>>,
    /// Selected table style in the family browser.
    pub editing_table_style: Signal<Option<String>>,
    /// Selected list style in the family browser.
    pub editing_list_style: Signal<Option<String>>,
    /// Per-level list-style draft (§10 tier 5) — `Some` mounts the level form.
    pub editing_list_level: Signal<Option<super::editor_style_editor::ListLevelDraftHandle>>,
    /// Selected page style in the family browser.
    pub editing_page_style: Signal<Option<String>>,
    /// Compact-only Edit/Inspect switch position.
    pub style_panel_inspect: Signal<bool>,
}

/// Renders whichever style panels are currently open, in flow.
#[allow(clippy::too_many_arguments)]
pub(super) fn style_panels(
    doc_state_picker: Arc<Mutex<DocumentState>>,
    doc_state_editor: Arc<Mutex<DocumentState>>,
    state: StylePanelState,
    // Table-style draft — its type is private to `editor_style_editor`, so it
    // is threaded through rather than named here.
    editing_table_draft: Signal<Option<super::editor_style_editor::TableStyleDraftHandle>>,
    // The style applied at the cursor, shown as the picker's current entry.
    current_style_name: String,
    // Size class, read once by the caller — the panel hosts no hooks.
    breakpoint: Breakpoint,
    // Font families enumerated on this device (memoised by the caller).
    font_families: Rc<Vec<String>>,
    // Loro / undo plumbing.
    sync: StyleEditorSync,
) -> Element {
    let doc_state_picker_char = Arc::clone(&doc_state_picker);
    rsx! {
        if *state.is_style_picker_open.read() {
            {style_picker_panel(
                doc_state_picker,
                sync.loro_doc,
                sync.cursor_state,
                sync.undo_manager,
                sync.can_undo,
                sync.can_redo,
                current_style_name,
                state.is_style_picker_open,
                state.style_search_query,
            )}
        }

        if *state.is_char_style_picker_open.read() {
            {super::editor_char_style_picker::char_style_picker_panel(
                Arc::clone(&doc_state_picker_char),
                sync.loro_doc,
                sync.cursor_state,
                sync.undo_manager,
                sync.can_undo,
                sync.can_redo,
                state.is_char_style_picker_open,
            )}
        }

        if state.editing_style_draft.read().is_some() {
            {style_editor_panel(
                doc_state_editor,
                state.editing_style_draft,
                state.editing_char_style,
                state.editing_char_draft,
                state.editing_table_style,
                editing_table_draft,
                state.editing_list_style,
                state.editing_list_level,
                state.editing_page_style,
                state.style_panel_inspect,
                breakpoint,
                font_families,
                sync,
            )}
        }
    }
}
