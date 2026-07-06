// SPDX-License-Identifier: Apache-2.0

//! Tests for the pure post-op caret math (plan 4a.2 follow-on). A structural
//! table edit shifts the flat cell indexing, so the caret's new cell must be
//! recomputed from its (row, col).

use super::{TableOp, caret_flat_after};

#[test]
fn insert_row_below_keeps_the_caret_cell() {
    // 2×2, caret at (row 0, col 1) → flat 1. Inserting below leaves it at 1.
    assert_eq!(caret_flat_after(TableOp::InsertRowBelow, 0, 1, 2, 2), 1);
    // Caret in the last row stays put too.
    assert_eq!(caret_flat_after(TableOp::InsertRowBelow, 1, 0, 2, 2), 2);
}

#[test]
fn insert_row_above_shifts_the_caret_down_a_row() {
    // 2×2, caret at (row 0, col 1) → flat 1. A new row above pushes it to
    // row 1, col 1 → flat 3.
    assert_eq!(caret_flat_after(TableOp::InsertRowAbove, 0, 1, 2, 2), 3);
    // Caret in row 1 → row 2, col 0 → flat 4.
    assert_eq!(caret_flat_after(TableOp::InsertRowAbove, 1, 0, 2, 2), 4);
}

#[test]
fn insert_column_right_widens_the_flat_index_for_later_rows() {
    // 2×2 → 2×3. Row 0 col 1 stays flat 1; row 1 col 1 moves 3 → 4.
    assert_eq!(caret_flat_after(TableOp::InsertColumnRight, 0, 1, 2, 2), 1);
    assert_eq!(caret_flat_after(TableOp::InsertColumnRight, 1, 1, 2, 2), 4);
}

#[test]
fn insert_column_left_shifts_the_caret_one_column_right() {
    // 2×2 → 2×3. Row 0 col 1 → col 2 (flat 2); row 1 col 1 → col 2 (flat 5).
    assert_eq!(caret_flat_after(TableOp::InsertColumnLeft, 0, 1, 2, 2), 2);
    assert_eq!(caret_flat_after(TableOp::InsertColumnLeft, 1, 1, 2, 2), 5);
}

#[test]
fn delete_row_lands_in_the_replacement_row() {
    // 3×2 delete row 1: the row that shifts up takes index 1, same column.
    assert_eq!(caret_flat_after(TableOp::DeleteRow, 1, 0, 3, 2), 2);
    // Deleting the last row clamps to the new last row.
    assert_eq!(caret_flat_after(TableOp::DeleteRow, 2, 1, 3, 2), 3);
}

#[test]
fn delete_column_lands_in_the_replacement_column() {
    // 2×3 delete col 1: new width 2. Row 0 col 1 → col 1 (flat 1);
    // row 1 col 1 → col 1 (flat 3).
    assert_eq!(caret_flat_after(TableOp::DeleteColumn, 0, 1, 2, 3), 1);
    assert_eq!(caret_flat_after(TableOp::DeleteColumn, 1, 1, 2, 3), 3);
    // Deleting the last column clamps to the new last column.
    assert_eq!(caret_flat_after(TableOp::DeleteColumn, 1, 2, 2, 3), 3);
}
