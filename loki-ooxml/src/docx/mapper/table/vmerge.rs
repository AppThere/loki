// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Vertical-merge two-pass algorithm.

use std::collections::{HashMap, HashSet};

use crate::docx::model::styles::{DocxTableRow, DocxVMerge};

/// Computes `row_span` values for all vertically-merged cells in a table.
///
/// Returns:
/// - `span_map`: `(row_idx, grid_col)` → `row_span` for every `Restart` cell.
///   The key uses the cell's *starting* grid column (accounting for
///   `w:gridSpan` of preceding cells in the same row).
/// - `skip_set`: `(row_idx, cell_idx)` pairs that are `Continue` cells and
///   should be omitted from the output row.
///
/// OOXML §17.4.84: `w:vMerge` with no `w:val` is a continuation cell.
///
/// # Algorithm
///
/// **Pass 1** — build a `v_merge_grid[row][grid_col]` by expanding each cell
/// by its `w:gridSpan` so that multi-column cells fill multiple grid slots
/// with the same vMerge state.
///
/// **Pass 2** — for each grid column, scan down; on every `Restart` cell,
/// count consecutive `Continue` cells below and record the span length.
/// Each counted `Continue` cell is added to `skip_set`.
#[allow(clippy::type_complexity)] // Pre-existing pattern — structural refactor deferred
pub(crate) fn compute_v_merge_spans(
    rows: &[DocxTableRow],
) -> (HashMap<(usize, usize), u32>, HashSet<(usize, usize)>) {
    // Pass 1: expand cells into a flat grid indexed by grid column.
    let mut v_merge_grid: Vec<Vec<Option<DocxVMerge>>> = Vec::with_capacity(rows.len());
    let mut cell_idx_grid: Vec<Vec<usize>> = Vec::with_capacity(rows.len());

    for row in rows {
        let mut v_merge_row: Vec<Option<DocxVMerge>> = Vec::new();
        let mut cell_idx_row: Vec<usize> = Vec::new();
        for (cell_idx, cell) in row.cells.iter().enumerate() {
            let v_merge = cell.tc_pr.as_ref().and_then(|p| p.v_merge);
            let col_span = cell
                .tc_pr
                .as_ref()
                .and_then(|p| p.grid_span)
                .unwrap_or(1)
                .max(1) as usize;
            for _ in 0..col_span {
                v_merge_row.push(v_merge);
                cell_idx_row.push(cell_idx);
            }
        }
        v_merge_grid.push(v_merge_row);
        cell_idx_grid.push(cell_idx_row);
    }

    let num_rows = v_merge_grid.len();
    let num_cols = v_merge_grid.iter().map(Vec::len).max().unwrap_or(0);

    let mut span_map: HashMap<(usize, usize), u32> = HashMap::new();
    // COMPAT(microsoft): w:vMerge with no w:val attribute is a continuation
    // cell per OOXML §17.4.84, not a restart. Some non-Microsoft producers
    // incorrectly omit w:vMerge entirely for continuation cells — those will
    // still render as row_span=1.
    let mut skip_set: HashSet<(usize, usize)> = HashSet::new();

    // Pass 2: for each column, find restart cells and count their span.
    for col in 0..num_cols {
        for row in 0..num_rows {
            if v_merge_grid[row].get(col).copied() == Some(Some(DocxVMerge::Restart)) {
                let mut span = 1u32;
                let mut r = row + 1;
                while r < num_rows
                    && v_merge_grid[r].get(col).copied() == Some(Some(DocxVMerge::Continue))
                {
                    if let Some(&cell_idx) = cell_idx_grid[r].get(col) {
                        skip_set.insert((r, cell_idx));
                    }
                    span += 1;
                    r += 1;
                }
                span_map.insert((row, col), span);
            }
        }
    }

    (span_map, skip_set)
}
