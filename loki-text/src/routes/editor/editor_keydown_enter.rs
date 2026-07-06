// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Enter-key handling for the document canvas: replace the active selection (if
//! any) with a paragraph break, then split the paragraph at the caret.
//!
//! Extracted from `editor_keydown_ctrl.rs` to keep that file under the 300-line
//! ceiling. Called by [`super::editor_keydown::make_keydown_handler`].

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::{StyleId, get_block_style_name, set_block_style, split_block_at};

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::{delete_selection_in_doc, set_collapsed_cursor};
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Handles the Enter key: replaces the active selection (if any) with a
/// paragraph break — splitting the current paragraph at the cursor position.
///
/// Like replace-typing (`handle_character_key`), an active selection is deleted
/// first and the split happens at the collapsed start, all in one relayout +
/// undo entry. A range the model rejects swallows the key.
pub(super) fn handle_enter_key(
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    let ldoc_guard = loro_doc.read();
    let Some(ldoc) = ldoc_guard.as_ref() else {
        return;
    };

    // Replace the active selection: delete it in the CRDT first (batched into
    // this same relayout/commit), then split at the collapsed start.
    let focus = if cursor_state.read().has_selection() {
        let Some(pos) = delete_selection_in_doc(ldoc, &cursor_state.read()) else {
            return; // rejected range — swallow the key, do not split
        };
        pos
    } else {
        focus
    };

    let nested = !focus.path.is_empty();

    // Resolve next_style_id for the current block's style before splitting.
    // Style inheritance via next_style_id is a top-level concern (named styles
    // address top-level paragraphs); nested splits keep the source block's type.
    let next_style: Option<String> = if nested {
        None
    } else {
        let style_name = get_block_style_name(ldoc, focus.paragraph_index);
        doc_state.lock().ok().and_then(|state| {
            state
                .document
                .as_ref()?
                .styles
                .paragraph_styles
                .get(&StyleId::new(&style_name))
                .and_then(|s| s.next_style_id.clone())
        })
    };

    if split_block_at(ldoc, &focus.block_path(), focus.byte_offset).is_err() {
        return;
    }

    // Apply the next_style to the newly created block if one is defined.
    if let Some(ref nstyle) = next_style {
        let _ = set_block_style(ldoc, focus.paragraph_index + 1, nstyle);
    }

    apply_mutation_and_relayout(doc_state, ldoc);
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    // The split inserts the new block right after the source within the same
    // container, so the caret moves to the next sibling block at offset 0
    // (its page_index is re-derived from the fresh layout — the new
    // paragraph may start on the next page).
    let new_pos = focus.sibling_block(1, 0);
    set_collapsed_cursor(doc_state, cursor_state, new_pos);
}
