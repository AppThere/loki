// SPDX-License-Identifier: Apache-2.0

//! Delete-table action for the Table contextual tab (Spec 04 M5).
//!
//! Extracted from `editor_ribbon_table` so that file stays under the 300-line
//! ceiling once its groups moved to the collapse-cascade `RibbonGroupSpec` form.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::delete_block;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::set_collapsed_cursor;
use super::editor_ribbon_table::block_count;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Deletes the table the caret is inside (its root block) and re-homes the
/// caret to the block that takes its place (or the previous block if it was
/// last), at offset 0.
pub(super) fn delete_current_table(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    // The table is the caret's root block (works from inside any of its cells).
    let Some(root) = cursor_state
        .peek()
        .focus
        .as_ref()
        .map(|f| f.paragraph_index)
    else {
        return;
    };
    // Re-check the guard at click time (the document may have changed).
    if block_count(doc_state) <= 1 {
        return;
    }
    {
        let guard = loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        if delete_block(ldoc, root).is_err() {
            return;
        }
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
    // `remaining >= 1` (guarded above), so this index is valid; the page is
    // re-derived from the fresh layout.
    let remaining = block_count(doc_state);
    let target = root.min(remaining.saturating_sub(1));
    set_collapsed_cursor(
        doc_state,
        cursor_state,
        DocumentPosition::top_level(0, target, 0),
    );
}
