// SPDX-License-Identifier: Apache-2.0

//! Ctrl/Meta/Super+key shortcut dispatch for the document canvas.
//!
//! Extracted from `editor_keydown.rs` to keep that file under the 300-line
//! ceiling.  Called by [`super::editor_keydown::make_keydown_handler`].

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use keyboard_types::{Key, Modifiers};
use loki_doc_model::loro_mutation::{delete_text_at, get_block_text, get_block_text_at};

use super::editor_formatting;
use crate::editing::cursor::{CursorState, DocumentPosition, next_grapheme_boundary};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Re-derives the `page_index` of the cursor's anchor and focus from the
/// current paginated layout. Used after undo/redo, which can move (or remove)
/// the caret's paragraph across a page boundary; `recompute_page_index` leaves
/// a position unchanged when its paragraph is no longer in the layout.
fn recompute_cursor_pages(
    doc_state: &Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
) {
    let Some(layout) = doc_state
        .lock()
        .ok()
        .and_then(|s| s.paginated_layout.clone())
    else {
        return;
    };
    let mut cs = cursor_state.write();
    if let Some(f) = cs.focus.clone() {
        cs.focus = Some(crate::editing::page_locate::recompute_page_index(
            &layout, &f,
        ));
    }
    if let Some(a) = cs.anchor.clone() {
        cs.anchor = Some(crate::editing::page_locate::recompute_page_index(
            &layout, &a,
        ));
    }
}

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
                let first = DocumentPosition::top_level(0, 0, 0);
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
                    let last = DocumentPosition::top_level(last_page, last_block, end_offset);
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
            drop(ldoc_guard);
            recompute_cursor_pages(doc_state, cursor_state);
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
            drop(ldoc_guard);
            recompute_cursor_pages(doc_state, cursor_state);
        }
        _ => {}
    }
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
}

/// Handles the forward-delete key: removes the grapheme at the cursor position.
pub(super) fn handle_delete_key(
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    let text = {
        let ldoc_guard = loro_doc.read();
        ldoc_guard
            .as_ref()
            .map(|l| get_block_text_at(l, &focus.block_path()))
            .unwrap_or_default()
    };
    if focus.byte_offset >= text.len() {
        return;
    }
    let next = next_grapheme_boundary(&text, focus.byte_offset);
    let len = next - focus.byte_offset;
    {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return;
        };
        if delete_text_at(ldoc, &focus.block_path(), focus.byte_offset, len).is_err() {
            return;
        }
    }
    {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return;
        };
        apply_mutation_and_relayout(doc_state, ldoc);
    }
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    // The caret byte is unchanged, but forward-deleting text from a
    // page-spanning paragraph can pull its later lines back a page — re-derive
    // the caret's page_index from the fresh layout (plan 4b.1).
    super::editor_keydown_text::set_collapsed_cursor(doc_state, cursor_state, focus);
}

/// Syncs cursor generation, `can_undo`, and `can_redo` after any document mutation.
///
/// Calls `loro_doc.commit()` before syncing so that each user action (bold
/// toggle, character insert, etc.) becomes its own discrete entry on the
/// `UndoManager` stack.  Without an explicit commit, multiple rapid mutations
/// may be merged into a single undo step.
///
/// Writing `cursor_state.document_generation` changes the `data-cursor` canvas
/// attribute, which causes Blitz to mark the node dirty and re-call `render()`.
/// Without this, formatting changes that do not move the cursor would have no
/// visible effect.
pub(super) fn post_mutation_sync(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    mut can_undo: Signal<bool>,
    mut can_redo: Signal<bool>,
) {
    // Commit the pending Loro transaction so this mutation becomes its own
    // discrete undo entry.
    if let Some(ldoc) = loro_doc.read().as_ref() {
        ldoc.commit();
    }
    if let Ok(s) = doc_state.lock() {
        cursor_state.write().document_generation = s.generation;
    }
    let um_guard = undo_manager.read();
    if let Some(um) = um_guard.as_ref() {
        can_undo.set(um.can_undo());
        can_redo.set(um.can_redo());
    }
}
