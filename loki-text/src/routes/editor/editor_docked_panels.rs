// SPDX-License-Identifier: Apache-2.0

//! Transient panels docked above the ribbon, triggered by toolbar actions:
//! the spelling suggestions menu, the spelling language picker, and the Insert
//! tab's hyperlink URL panel.
//!
//! Bundled into one element so `editor_inner` (an oversized file) stays lean.
//! Each sub-panel self-gates on its own open/draft signal, so this helper can be
//! rendered unconditionally. None of these use `position: absolute` except the
//! spelling menu (verified to work in the current Blitz stack).

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;

use super::editor_insert_panel::{InsertLinkSync, insert_link_panel};
use super::editor_language_panel::language_panel;
use super::editor_spell::{SpellMenu, SpellSync};
use super::editor_spell_panel::spelling_panel;
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// Signals shared by the docked panels (the document handle and undo/dirty
/// tracking). Mirrors the per-panel `*Sync` structs, built once by the caller.
#[derive(Clone, Copy)]
pub(super) struct DockedSync {
    /// The document's Loro CRDT handle.
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    /// Cursor state (mirrors the document generation for dirty tracking).
    pub cursor_state: Signal<CursorState>,
    /// Undo manager, refreshed after each mutation.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether undo is available.
    pub can_undo: Signal<bool>,
    /// Whether redo is available.
    pub can_redo: Signal<bool>,
}

/// Renders the spelling suggestions menu, the language picker, and the Insert
/// hyperlink panel. Each self-gates on its trigger signal.
#[allow(clippy::too_many_arguments)]
pub(super) fn docked_panels(
    doc_state: Arc<Mutex<DocumentState>>,
    sync: DockedSync,
    spell_service: SpellService,
    spell_menu: Signal<Option<SpellMenu>>,
    is_language_panel_open: Signal<bool>,
    language_status: Signal<Option<String>>,
    spell_hover: Signal<Option<String>>,
    client_width: f32,
    link_draft: Signal<Option<String>>,
) -> Element {
    let ds_lang = Arc::clone(&doc_state);
    let ds_link = Arc::clone(&doc_state);
    let spell_sync = SpellSync {
        loro_doc: sync.loro_doc,
        cursor_state: sync.cursor_state,
        undo_manager: sync.undo_manager,
        can_undo: sync.can_undo,
        can_redo: sync.can_redo,
    };
    rsx! {
        if spell_menu.read().is_some() {
            {spelling_panel(
                doc_state,
                spell_sync,
                spell_service.clone(),
                spell_menu,
                is_language_panel_open,
                client_width,
                spell_hover,
            )}
        }
        if is_language_panel_open() {
            {language_panel(
                ds_lang,
                sync.cursor_state,
                spell_service,
                is_language_panel_open,
                language_status,
            )}
        }
        if link_draft.read().is_some() {
            {insert_link_panel(
                ds_link,
                link_draft,
                InsertLinkSync {
                    loro_doc: sync.loro_doc,
                    cursor_state: sync.cursor_state,
                    undo_manager: sync.undo_manager,
                    can_undo: sync.can_undo,
                    can_redo: sync.can_redo,
                },
            )}
        }
    }
}
