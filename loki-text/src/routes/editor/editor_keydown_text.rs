// SPDX-License-Identifier: Apache-2.0

//! Printable-character and Backspace handling for the document canvas,
//! including selection-aware replacement and removal (audit F6c): typing
//! replaces the active selection, Backspace/Delete remove it.
//!
//! Extracted from `editor_keydown.rs` to keep that file under the 300-line
//! ceiling.  Called by [`super::editor_keydown::make_keydown_handler`].

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::loro_mutation::{delete_text_at, get_block_text_at, insert_text_at};
use loki_doc_model::{PathStep, delete_selection_at, merge_block_at};

use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::cursor::{CursorState, DocumentPosition, prev_grapheme_boundary};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

#[cfg(test)]
#[path = "editor_keydown_text_tests.rs"]
mod tests;

/// What [`remove_selection`] did with the active selection.
pub(super) enum SelectionRemoval {
    /// No range selection was active — the caller performs its normal
    /// single-cursor action.
    NoSelection,
    /// The selected range was deleted and the cursor collapsed to its start.
    Removed,
    /// A selection was active but the model rejected the range (endpoints in
    /// different containers, or a non-text block inside it). Nothing was
    /// mutated; the caller must NOT fall through to a single-cursor edit —
    /// swallowing the key beats surprising the user with a stray deletion.
    Rejected,
}

/// The selection endpoint that comes first in document order, using the same
/// `(leaf block index, byte offset)` normalization as
/// [`delete_selection_at`] — so the collapsed cursor keeps the right
/// `page_index`.
fn selection_start(a: &DocumentPosition, b: &DocumentPosition) -> DocumentPosition {
    fn leaf(p: &DocumentPosition) -> usize {
        match p.path.last() {
            Some(PathStep::Cell { block, .. } | PathStep::Note { block, .. }) => *block,
            None => p.paragraph_index,
        }
    }
    if (leaf(a), a.byte_offset) <= (leaf(b), b.byte_offset) {
        a.clone()
    } else {
        b.clone()
    }
}

/// Deletes the active selection in the CRDT only — no relayout or undo
/// commit, so a caller can batch a follow-up insert into the same undo entry.
///
/// Returns the collapsed cursor position, or `None` when there is no active
/// selection or the model rejected the range (nothing mutated either way).
fn delete_selection_in_doc(ldoc: &loro::LoroDoc, cursor: &CursorState) -> Option<DocumentPosition> {
    if !cursor.has_selection() {
        return None;
    }
    let (anchor, focus) = (cursor.anchor.clone()?, cursor.focus.clone()?);
    let (_, byte) = delete_selection_at(
        ldoc,
        (&anchor.block_path(), anchor.byte_offset),
        (&focus.block_path(), focus.byte_offset),
    )
    .ok()?;
    Some(DocumentPosition {
        byte_offset: byte,
        ..selection_start(&anchor, &focus)
    })
}

/// Removes the active selection (Backspace/Delete over a range): mutates,
/// relayouts, syncs, and collapses the cursor to the range start.
#[allow(clippy::too_many_arguments)] // mirrors the other keydown helpers' signals
pub(super) fn remove_selection(
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> SelectionRemoval {
    if !cursor_state.read().has_selection() {
        return SelectionRemoval::NoSelection;
    }
    let collapsed = {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return SelectionRemoval::Rejected;
        };
        let Some(pos) = delete_selection_in_doc(ldoc, &cursor_state.read()) else {
            return SelectionRemoval::Rejected;
        };
        apply_mutation_and_relayout(doc_state, ldoc);
        pos
    };
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    let mut cs = cursor_state.write();
    cs.focus = Some(collapsed.clone());
    cs.anchor = Some(collapsed);
    SelectionRemoval::Removed
}

/// Handles a printable character: replaces the active selection (if any),
/// inserts the character, and places the caret after it.
///
/// The selection delete and the insert share one relayout + commit, so
/// replace-typing is a single undo entry.
#[allow(clippy::too_many_arguments)] // mirrors the other keydown helpers' signals
pub(super) fn handle_character_key(
    ch: String,
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    let insert_at = {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return;
        };
        let insert_at = if cursor_state.read().has_selection() {
            // Replace-typing. A rejected range swallows the keystroke.
            let Some(pos) = delete_selection_in_doc(ldoc, &cursor_state.read()) else {
                return;
            };
            pos
        } else {
            focus
        };
        if insert_text_at(ldoc, &insert_at.block_path(), insert_at.byte_offset, &ch).is_err() {
            return;
        }
        apply_mutation_and_relayout(doc_state, ldoc);
        insert_at
    };
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    let new_pos = DocumentPosition {
        byte_offset: insert_at.byte_offset + ch.len(),
        ..insert_at
    };
    let mut cs = cursor_state.write();
    cs.focus = Some(new_pos.clone());
    cs.anchor = Some(new_pos);
}

/// Handles Backspace: removes the active selection, or merges with the
/// previous block at offset 0, or deletes the previous grapheme.
#[allow(clippy::too_many_arguments)] // mirrors the other keydown helpers' signals
pub(super) fn handle_backspace_key(
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
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
        // Backspace-at-start merges this block into its previous sibling
        // within the same container. `merge_block_at` returns
        // `NoPreviousBlock` at the first block of a container (a top-level
        // paragraph 0 or the first block of a cell / note body), making this
        // a no-op there.
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
        // TODO(3b-3): recompute page_index from layout after merge.
        // Caret lands at the join point in the previous sibling block.
        let new_pos = focus.sibling_block(-1, merged_offset);
        let mut cs = cursor_state.write();
        cs.focus = Some(new_pos.clone());
        cs.anchor = Some(new_pos);
        return;
    }
    let prev = {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return;
        };
        let text = get_block_text_at(ldoc, &focus.block_path());
        let prev = prev_grapheme_boundary(&text, focus.byte_offset);
        let len = focus.byte_offset - prev;
        if delete_text_at(ldoc, &focus.block_path(), prev, len).is_err() {
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
    let new_pos = DocumentPosition {
        byte_offset: prev,
        ..focus
    };
    let mut cs = cursor_state.write();
    cs.focus = Some(new_pos.clone());
    cs.anchor = Some(new_pos);
}
