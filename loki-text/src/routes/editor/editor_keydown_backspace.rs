// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Backspace handling for the document canvas, including paragraph-mark tracked
//! deletion (Review tab 4a.2). Split from `editor_keydown_text` to hold the
//! 300-line ceiling.
//!
//! Backspace resolves in order: remove the active selection; at a paragraph
//! start, either **record a tracked ¶-deletion** (track changes on, top-level)
//! or merge into the previous block; otherwise delete the previous grapheme
//! (struck when tracking is on).

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::loro_mutation::{
    get_block_text_at, set_para_mark_deletion, tracked_grapheme_delete,
};
use loki_doc_model::style::props::revision::RevisionMark;
use loki_doc_model::{BlockPath, merge_block_at};

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::{
    SelectionRemoval, deletion_mark, remove_selection, set_collapsed_cursor,
};
use crate::editing::cursor::{CursorState, DocumentPosition, prev_grapheme_boundary};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Handles Backspace: removes the active selection, records/merges at a
/// paragraph start, or deletes the previous grapheme.
#[allow(clippy::too_many_arguments)] // mirrors the other keydown helpers' signals
pub(super) fn handle_backspace_key(
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    match remove_selection(
        loro_doc,
        doc_state,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    ) {
        SelectionRemoval::Removed | SelectionRemoval::Rejected => return,
        SelectionRemoval::NoSelection => {}
    }
    if focus.byte_offset == 0 {
        // With track changes on, Backspace at the start of a top-level paragraph
        // records a tracked deletion of the *previous* paragraph's mark (¶)
        // instead of merging; accept-all performs the merge later. A nested
        // container or a non-paragraph previous block falls back to a hard merge
        // (TODO(review-para-mark-nested)).
        if let Some(rev) = deletion_mark(doc_state)
            && focus.path.is_empty()
            && focus.paragraph_index > 0
            && record_para_mark_deletion(
                &focus,
                rev,
                loro_doc,
                doc_state,
                cursor_state,
                undo_manager,
                can_undo,
                can_redo,
            )
        {
            return;
        }
        merge_previous_block(
            &focus,
            loro_doc,
            doc_state,
            cursor_state,
            undo_manager,
            can_undo,
            can_redo,
        );
        return;
    }
    delete_previous_grapheme(
        focus,
        loro_doc,
        doc_state,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
}

/// Records a tracked ¶-deletion on the previous top-level paragraph, moving the
/// caret to that paragraph's end. Returns `false` (recording nothing) when the
/// previous block is not a paragraph, so the caller hard-merges instead.
#[allow(clippy::too_many_arguments)]
fn record_para_mark_deletion(
    focus: &DocumentPosition,
    rev: RevisionMark,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> bool {
    let prev = focus.paragraph_index - 1;
    let prev_len = {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return false;
        };
        match set_para_mark_deletion(ldoc, prev, &rev) {
            Ok(true) => {
                apply_mutation_and_relayout(doc_state, ldoc);
                get_block_text_at(ldoc, &BlockPath::block(prev)).len()
            }
            // Declined (non-paragraph previous) or error — let the caller merge.
            _ => return false,
        }
    };
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    set_collapsed_cursor(doc_state, cursor_state, focus.sibling_block(-1, prev_len));
    true
}

/// Merges the block at `focus` into its previous sibling (Backspace-at-start with
/// tracking off, or a nested / non-paragraph merge). `merge_block_at` is a no-op
/// (`NoPreviousBlock`) at the first block of a container.
#[allow(clippy::too_many_arguments)]
fn merge_previous_block(
    focus: &DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    let merged_offset = {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return;
        };
        let Ok(merged_offset) = merge_block_at(ldoc, &focus.block_path()) else {
            return;
        };
        apply_mutation_and_relayout(doc_state, ldoc);
        merged_offset
    };
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    set_collapsed_cursor(
        doc_state,
        cursor_state,
        focus.sibling_block(-1, merged_offset),
    );
}

/// Deletes the grapheme before the caret, striking it through when track changes
/// is on (`tracked_grapheme_delete` decides). The caret lands before it.
#[allow(clippy::too_many_arguments)]
fn delete_previous_grapheme(
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    let del_rev = deletion_mark(doc_state);
    let prev = {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return;
        };
        let text = get_block_text_at(ldoc, &focus.block_path());
        let prev = prev_grapheme_boundary(&text, focus.byte_offset);
        let path = focus.block_path();
        if tracked_grapheme_delete(ldoc, &path, prev, focus.byte_offset, del_rev.as_ref()).is_err()
        {
            return;
        }
        apply_mutation_and_relayout(doc_state, ldoc);
        prev
    };
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    set_collapsed_cursor(
        doc_state,
        cursor_state,
        DocumentPosition {
            byte_offset: prev,
            ..focus
        },
    );
}
