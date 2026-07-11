// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Printable-character and selection handling for the document canvas (audit
//! F6c): typing replaces the active selection; [`remove_selection`] deletes (or
//! strikes) it for Backspace/Delete. Backspace's block/grapheme paths live in
//! [`super::editor_keydown_backspace`]. Called by
//! [`super::editor_keydown::make_keydown_handler`].

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::PathStep;
use loki_doc_model::loro_mutation::{
    insert_text_at, insert_text_tracked_at, tracked_delete_selection_at,
};
use loki_doc_model::style::props::revision::RevisionMark;

use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

#[cfg(test)]
#[path = "editor_keydown_text_tests.rs"]
mod tests;

/// Collapses the cursor to `pos` after a mutation, re-deriving its `page_index`
/// from the freshly relaid-out layout (a keystroke near a page boundary can move
/// the caret's paragraph to a different page, plan 4b.1).
pub(super) fn set_collapsed_cursor(
    doc_state: &Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
    pos: DocumentPosition,
) {
    let layout = doc_state
        .lock()
        .ok()
        .and_then(|s| s.paginated_layout.clone());
    let pos = match layout {
        Some(l) => crate::editing::page_locate::recompute_page_index(&l, &pos),
        None => pos,
    };
    let mut cs = cursor_state.write();
    cs.focus = Some(pos.clone());
    cs.anchor = Some(pos);
}

/// What [`remove_selection`] did with the active selection.
pub(super) enum SelectionRemoval {
    /// No range selection was active — the caller does its single-cursor action.
    NoSelection,
    /// The selected range was deleted (or struck) and the cursor collapsed.
    Removed,
    /// A selection was active but the model rejected the range (cross-container,
    /// or a non-text block inside it); nothing was mutated. The caller must NOT
    /// fall through to a single-cursor edit — swallowing the key beats a stray
    /// deletion.
    Rejected,
}

/// The selection endpoint first in document order (same `(leaf, byte)`
/// normalization as `delete_selection_at`), so the collapsed cursor keeps the
/// right `page_index`.
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

/// Deletes the active selection in the CRDT only (no relayout/commit, so a
/// caller can batch a follow-up insert or split into the same undo entry). With
/// track changes on, `deletion` is the author's mark and the selection is struck
/// through instead of removed; with it `None` the range is hard-deleted. Returns
/// the collapsed cursor, or `None` when there is no selection or the range was
/// rejected (nothing mutated).
pub(super) fn delete_selection_in_doc(
    ldoc: &loro::LoroDoc,
    cursor: &CursorState,
    deletion: Option<&RevisionMark>,
) -> Option<DocumentPosition> {
    if !cursor.has_selection() {
        return None;
    }
    let (anchor, focus) = (cursor.anchor.clone()?, cursor.focus.clone()?);
    let (_, byte) = tracked_delete_selection_at(
        ldoc,
        (&anchor.block_path(), anchor.byte_offset),
        (&focus.block_path(), focus.byte_offset),
        deletion,
    )
    .ok()?;
    Some(DocumentPosition {
        byte_offset: byte,
        ..selection_start(&anchor, &focus)
    })
}

/// Reads the document's tracked-deletion mark (present iff track changes is on).
pub(super) fn deletion_mark(doc_state: &Arc<Mutex<DocumentState>>) -> Option<RevisionMark> {
    doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().and_then(|d| d.deletion_revision()))
}

/// Removes the active selection (Backspace/Delete over a range): mutates,
/// relayouts, syncs, and collapses the cursor to the range start.
#[allow(clippy::too_many_arguments)] // mirrors the other keydown helpers' signals
pub(super) fn remove_selection(
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> SelectionRemoval {
    if !cursor_state.read().has_selection() {
        return SelectionRemoval::NoSelection;
    }
    let deletion = deletion_mark(doc_state);
    let collapsed = {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return SelectionRemoval::Rejected;
        };
        let Some(pos) = delete_selection_in_doc(ldoc, &cursor_state.read(), deletion.as_ref())
        else {
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
    set_collapsed_cursor(doc_state, cursor_state, collapsed);
    SelectionRemoval::Removed
}

/// Handles a printable character: replaces the active selection (if any),
/// inserts the character, and places the caret after it. The selection delete
/// and the insert share one relayout + commit (a single undo entry).
#[allow(clippy::too_many_arguments)] // mirrors the other keydown helpers' signals
pub(super) fn handle_character_key(
    ch: String,
    focus: DocumentPosition,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    // With track changes on, stamp typed text as a tracked insertion (else plain).
    let revision = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().and_then(|d| d.insertion_revision()));
    // Replace-typing over a selection strikes the old text (Word inserts the new
    // run before the struck one).
    let deletion = deletion_mark(doc_state);
    let insert_at = {
        let ldoc_guard = loro_doc.read();
        let Some(ldoc) = ldoc_guard.as_ref() else {
            return;
        };
        let insert_at = if cursor_state.read().has_selection() {
            // Replace-typing. A rejected range swallows the keystroke.
            let Some(pos) = delete_selection_in_doc(ldoc, &cursor_state.read(), deletion.as_ref())
            else {
                return;
            };
            pos
        } else {
            focus
        };
        let path = insert_at.block_path();
        let inserted = match &revision {
            Some(rev) => insert_text_tracked_at(ldoc, &path, insert_at.byte_offset, &ch, rev),
            None => insert_text_at(ldoc, &path, insert_at.byte_offset, &ch),
        };
        if inserted.is_err() {
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
    set_collapsed_cursor(doc_state, cursor_state, new_pos);
}
