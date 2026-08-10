// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Enter-key handling for the document canvas: replace the active selection (if
//! any) with a paragraph break, then split the paragraph at the caret.
//!
//! Extracted from `editor_keydown_ctrl.rs` to keep that file under the 300-line
//! ceiling. Called by [`super::editor_keydown::make_keydown_handler`].

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::{
    clear_block_list, get_block_heading_style, get_block_list_id, get_block_style_name,
    get_block_text, set_block_style, set_block_type_heading, set_block_type_para, split_block_at,
};

use super::editor_next_style::{NextBlock, next_block_for_split};

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::{delete_selection_in_doc, deletion_mark, set_collapsed_cursor};
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

    let has_selection = cursor_state.read().has_selection();

    // Double-Enter exits a list: pressing Enter on an *empty*, top-level list
    // item removes its list formatting instead of inserting another bullet
    // (Word / LibreOffice behaviour, plan 4b.1). Only for a plain caret — a
    // selection means the user is replacing text, so fall through to the split.
    if is_empty_list_item_exit(ldoc, &focus, has_selection) {
        if clear_block_list(ldoc, focus.paragraph_index).is_err() {
            return;
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
        // The caret stays in the same (now non-list) paragraph at offset 0;
        // re-derive its page from the fresh layout (plan 4b.1).
        set_collapsed_cursor(doc_state, cursor_state, focus);
        return;
    }

    // Replace the active selection: delete it in the CRDT first (batched into
    // this same relayout/commit), then split at the collapsed start. With track
    // changes on the selection is struck (not removed) before the split.
    let focus = if has_selection {
        let deletion = deletion_mark(doc_state);
        let Some(pos) = delete_selection_in_doc(ldoc, &cursor_state.read(), deletion.as_ref())
        else {
            return; // rejected range — swallow the key, do not split
        };
        pos
    } else {
        focus
    };

    let nested = !focus.path.is_empty();

    // Resolve the next style for the tail block before splitting. Top-level
    // only (named styles address top-level paragraphs), and only when the
    // caret sits at the end of the paragraph — a mid-text split leaves both
    // halves in the source style (Word semantics; `editor_next_style`).
    let at_paragraph_end = focus.byte_offset >= get_block_text(ldoc, focus.paragraph_index).len();
    let next_block: Option<NextBlock> = if nested || !at_paragraph_end {
        None
    } else {
        let key = get_block_style_name(ldoc, focus.paragraph_index);
        let stored = get_block_heading_style(ldoc, focus.paragraph_index);
        doc_state
            .lock()
            .ok()
            .and_then(|state| next_block_for_split(&state.document.as_ref()?.styles, &key, stored))
    };

    if split_block_at(ldoc, &focus.block_path(), focus.byte_offset).is_err() {
        return;
    }

    // Apply the next style to the newly created block. A heading next style is
    // a block-type change; anything else converts the tail to a styled_para —
    // without the type reset, Enter after a heading would produce another
    // heading merely tagged with the body style's name.
    match next_block {
        Some(NextBlock::Heading(level)) => {
            let _ = set_block_type_heading(ldoc, focus.paragraph_index + 1, level);
        }
        Some(NextBlock::Styled(id)) => {
            // Retype first — the style write only lands on a styled_para.
            let _ = set_block_type_para(ldoc, focus.paragraph_index + 1)
                .and_then(|()| set_block_style(ldoc, focus.paragraph_index + 1, &id));
        }
        None => {}
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

/// Whether Enter should exit the list rather than split the paragraph: a plain
/// caret (no selection) sitting on an **empty, top-level list item** (plan
/// 4b.1). Reads only — the caller performs the `clear_block_list` mutation.
///
/// Nested list items (inside a table cell / note body) are excluded: the list
/// block API is top-level only, so they keep the normal split behaviour.
pub(super) fn is_empty_list_item_exit(
    ldoc: &loro::LoroDoc,
    focus: &DocumentPosition,
    has_selection: bool,
) -> bool {
    !has_selection
        && focus.path.is_empty()
        && get_block_list_id(ldoc, focus.paragraph_index).is_some()
        && get_block_text(ldoc, focus.paragraph_index).is_empty()
}

#[cfg(test)]
#[path = "editor_keydown_enter_tests.rs"]
mod tests;
