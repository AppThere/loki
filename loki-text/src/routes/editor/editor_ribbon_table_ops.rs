// SPDX-License-Identifier: Apache-2.0

//! Row/column operations for the Table contextual tab (plan 4a.2 follow-on).
//!
//! Each op derives its target row/column from the caret's cell, applies the
//! matching `loki_doc_model` structural mutation, and re-homes the caret to its
//! (possibly shifted) cell — a structural edit changes the flat cell indexing
//! the cursor path stores, so the caret must be recomputed, not left stale.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::{
    PathStep, delete_table_column, delete_table_row, insert_table_column, insert_table_row,
    table_grid_dims,
};

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::set_collapsed_cursor;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// A structural table edit driven from the caret's cell. Insert ops add a row
/// above/below or a column left/right of the caret; delete ops remove it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TableOp {
    InsertRowAbove,
    InsertRowBelow,
    DeleteRow,
    InsertColumnLeft,
    InsertColumnRight,
    DeleteColumn,
}

/// The caret's new flat cell index after `op` is applied to a `rows`×`cols`
/// grid, given the caret's current `(row, col)`. Assumes the op succeeded (so a
/// delete implies the deleted dimension had at least two entries).
pub(super) fn caret_flat_after(
    op: TableOp,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> usize {
    match op {
        // Insert below: (row, col) and the column count are unchanged.
        TableOp::InsertRowBelow => row * cols + col,
        // Insert above: the caret's row shifts down one; column count unchanged.
        TableOp::InsertRowAbove => (row + 1) * cols + col,
        // Insert to the right: (row, col) unchanged, the grid is one column wider.
        TableOp::InsertColumnRight => row * (cols + 1) + col,
        // Insert to the left: the caret shifts one column right in the wider grid.
        TableOp::InsertColumnLeft => row * (cols + 1) + col + 1,
        // The caret's row is gone; land in the row that takes its place (or the
        // new last row if it was the last).
        TableOp::DeleteRow => {
            let new_rows = rows - 1;
            let target_row = row.min(new_rows.saturating_sub(1));
            target_row * cols + col
        }
        // The caret's column is gone; land in the column that takes its place.
        TableOp::DeleteColumn => {
            let new_cols = cols - 1;
            let target_col = col.min(new_cols.saturating_sub(1));
            row * new_cols + target_col
        }
    }
}

/// The caret's `(table_index, flat_cell)` when it sits in a table cell.
fn caret_cell(cursor_state: Signal<CursorState>) -> Option<(usize, usize)> {
    let cs = cursor_state.peek();
    let focus = cs.focus.as_ref()?;
    let flat = focus.path.iter().find_map(|s| match s {
        PathStep::Cell { cell, .. } => Some(*cell),
        _ => None,
    })?;
    Some((focus.paragraph_index, flat))
}

/// Applies `op` to the table the caret is in, relays out, syncs undo/redo, and
/// re-homes the caret to its shifted cell. A no-op when the caret is not in a
/// simple-grid table cell or the mutation is rejected (e.g. deleting the last
/// row/column).
pub(super) fn run_table_op(
    op: TableOp,
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    let Some((table_index, flat)) = caret_cell(cursor_state) else {
        return;
    };
    let new_flat = {
        let guard = loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        let Some((rows, cols)) = table_grid_dims(ldoc, table_index) else {
            return;
        };
        let (row, col) = (flat / cols, flat % cols);
        let res = match op {
            TableOp::InsertRowAbove => insert_table_row(ldoc, table_index, row),
            TableOp::InsertRowBelow => insert_table_row(ldoc, table_index, row + 1),
            TableOp::DeleteRow => delete_table_row(ldoc, table_index, row),
            TableOp::InsertColumnLeft => insert_table_column(ldoc, table_index, col),
            TableOp::InsertColumnRight => insert_table_column(ldoc, table_index, col + 1),
            TableOp::DeleteColumn => delete_table_column(ldoc, table_index, col),
        };
        if res.is_err() {
            return;
        }
        apply_mutation_and_relayout(doc_state, ldoc);
        caret_flat_after(op, row, col, rows, cols)
    };
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    let mut pos = DocumentPosition::top_level(0, table_index, 0);
    pos.path = vec![PathStep::Cell {
        cell: new_flat,
        block: 0,
    }];
    set_collapsed_cursor(doc_state, cursor_state, pos);
}

#[cfg(test)]
#[path = "editor_ribbon_table_ops_tests.rs"]
mod tests;
