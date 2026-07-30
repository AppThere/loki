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
use super::editor_spell_popover::{SpellPopover, SpellPopoverProps};
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
///
/// # ADR-0013 is violated here, and by six siblings — Spec 08 I-27
///
/// Every panel below is mounted as `if cond { plain_function(..) }`, which
/// ADR-0013 forbids: only a component owns a hook scope, so a plain function
/// cannot call `use_breakpoint()` and cannot adapt its posture without a
/// `compact` flag threaded from here.
///
/// **Swept r62 rather than assumed local**: seven conditionally-mounted panels
/// are plain functions — spelling, language, insert-link, publish, metadata,
/// style-picker and style-editor — and `AtPanelHost`, the host the ADR names as
/// the sanctioned boundary, has **zero consumers** in this crate. So the ADR's
/// own remedy is unwired, which is the fourth instance of a capability that
/// reached nobody.
///
/// The cause is dates: ADR-0013 is 2026-06-30 and these panels predate it by a
/// day or more. **An ADR that arrives after the code needs a sweep, and nothing
/// swept.**
///
/// T4.1's spell-panel migration is the forcing function for the first one —
/// pushing a popover request is a signal write, which cannot happen during a
/// plain function's render — so that conversion is **compliance the migration
/// exposes, not new scope**. The other six are I-27 and not this session's work.
#[allow(clippy::too_many_arguments)]
pub(super) fn docked_panels(
    doc_state: Arc<Mutex<DocumentState>>,
    sync: DockedSync,
    spell_service: SpellService,
    spell_menu: Signal<Option<SpellMenu>>,
    is_language_panel_open: Signal<bool>,
    language_status: Signal<Option<String>>,
    spell_hover: Signal<Option<String>>,
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
        // ADR-0013 compliant, and the first of the seven (I-27): a component, so
        // it has the hook scope the popover handover needs.
        if spell_menu.read().is_some() {
            SpellPopover {
                ..SpellPopoverProps {
                    doc_state,
                    sync: spell_sync,
                    service: spell_service.clone(),
                    spell_menu,
                    is_language_panel_open,
                    spell_hover,
                }
            }
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
