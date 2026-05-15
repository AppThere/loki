// SPDX-License-Identifier: Apache-2.0

//! Ctrl/Meta/Super+key shortcut dispatch for the document canvas.
//!
//! Extracted from `editor_keydown.rs` to keep that file under the 300-line
//! ceiling.  Called by [`super::editor_keydown::make_keydown_handler`].

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use keyboard_types::{Key, Modifiers};
use loki_doc_model::loro_mutation::get_block_text;

use super::editor_formatting;
use crate::components::document_source::{DocumentState, apply_mutation_and_relayout};
use crate::editing::cursor::{CursorState, DocumentPosition};

/// Dispatches Ctrl/Meta/Super+key shortcuts.
///
/// Handles select-all (`a`), bold (`b`), italic (`i`), underline (`u`),
/// undo (`z`), redo (`y`, `Shift+z`).  Caller must `return` after this call
/// to skip normal key processing for any held modifier.
///
/// After any document-mutating key, `can_undo` and `can_redo` are synced from
/// the `UndoManager`.
#[allow(clippy::too_many_arguments)] // command-key dispatch requires all editor signals
pub(super) fn handle_ctrl_keys(
    doc_state: &Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    modifiers: Modifiers,
    key: &Key,
) {
    let Key::Character(ch) = key else { return };
    match ch.as_str() {
        "a" => {
            let layout_opt = {
                let state = doc_state.lock().unwrap_or_else(|e| e.into_inner());
                state.paginated_layout.clone()
            };
            if let Some(layout) = layout_opt {
                let first = DocumentPosition {
                    page_index: 0,
                    paragraph_index: 0,
                    byte_offset: 0,
                };
                let last_opt = layout
                    .pages
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(pi, page)| {
                        page.editing_data
                            .as_ref()?
                            .paragraphs
                            .iter()
                            .max_by_key(|p| p.block_index)
                            .map(|p| (pi, p.block_index))
                    });
                if let Some((last_page, last_block)) = last_opt {
                    let end_offset = loro_doc
                        .read()
                        .as_ref()
                        .map(|l| get_block_text(l, last_block).len())
                        .unwrap_or(0);
                    let last = DocumentPosition {
                        page_index: last_page,
                        paragraph_index: last_block,
                        byte_offset: end_offset,
                    };
                    let mut cs = cursor_state.write();
                    cs.anchor = Some(first);
                    cs.focus = Some(last);
                }
            }
        }
        "b" => {
            let ldoc_guard = loro_doc.read();
            if let Some(ldoc) = ldoc_guard.as_ref() {
                let _ = editor_formatting::toggle_bold(ldoc, &cursor_state.read());
                apply_mutation_and_relayout(doc_state, ldoc);
            }
        }
        "i" => {
            let ldoc_guard = loro_doc.read();
            if let Some(ldoc) = ldoc_guard.as_ref() {
                let _ = editor_formatting::toggle_italic(ldoc, &cursor_state.read());
                apply_mutation_and_relayout(doc_state, ldoc);
            }
        }
        "u" => {
            let ldoc_guard = loro_doc.read();
            if let Some(ldoc) = ldoc_guard.as_ref() {
                let _ = editor_formatting::toggle_underline(ldoc, &cursor_state.read());
                apply_mutation_and_relayout(doc_state, ldoc);
            }
        }
        "z" => {
            {
                let mut um_guard = undo_manager.write();
                if let Some(um) = um_guard.as_mut() {
                    if modifiers.shift() {
                        let _ = um.redo();
                    } else {
                        let _ = um.undo();
                    }
                }
            }
            let ldoc_guard = loro_doc.read();
            if let Some(ldoc) = ldoc_guard.as_ref() {
                apply_mutation_and_relayout(doc_state, ldoc);
            }
        }
        "y" => {
            {
                let mut um_guard = undo_manager.write();
                if let Some(um) = um_guard.as_mut() {
                    let _ = um.redo();
                }
            }
            let ldoc_guard = loro_doc.read();
            if let Some(ldoc) = ldoc_guard.as_ref() {
                apply_mutation_and_relayout(doc_state, ldoc);
            }
        }
        _ => {}
    }
    post_mutation_sync(doc_state, cursor_state, undo_manager, can_undo, can_redo);
}

/// Syncs cursor generation, `can_undo`, and `can_redo` after any document mutation.
///
/// Writing `cursor_state.document_generation` changes the `data-cursor` canvas
/// attribute, which causes Blitz to mark the node dirty and re-call `render()`.
/// Without this, formatting changes that do not move the cursor would have no
/// visible effect.
pub(super) fn post_mutation_sync(
    doc_state: &Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    mut can_undo: Signal<bool>,
    mut can_redo: Signal<bool>,
) {
    if let Ok(s) = doc_state.lock() {
        cursor_state.write().document_generation = s.generation;
    }
    let um_guard = undo_manager.read();
    if let Some(um) = um_guard.as_ref() {
        can_undo.set(um.can_undo());
        can_redo.set(um.can_redo());
    }
}
