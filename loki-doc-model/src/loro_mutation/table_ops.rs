// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Structural table mutations: insert/delete rows and columns.
//!
//! A table is stored as a serde **skeleton** (the whole structure with cell
//! blocks emptied) plus a flat `KEY_TABLE_CELLS` movable list — one live
//! block-list per cell, in head→bodies→foot row-major order (see
//! [`crate::loro_bridge::table`]). A structural edit must keep the two in sync:
//! after the edit the skeleton's flat cell count/order must still match the cell
//! list. To preserve each surviving cell's live CRDT text, the cell list is
//! **patched** (only the affected entries are inserted/removed), never rebuilt.
//!
//! Scope: **simple grid** tables — exactly one body, no head/foot rows, no
//! row/column spans, and a uniform column count (the shape Insert → Table
//! creates). Any other shape returns [`MutationError::UnsupportedTableStructure`]
//! so the caller can decline rather than corrupt the table.

use loro::{LoroDoc, LoroMap, LoroMovableList};

use super::{MutationError, get_block_map_and_list};
use crate::content::block::Block;
use crate::content::table::col::ColSpec;
use crate::content::table::core::Table;
use crate::content::table::row::{Cell, Row};
use crate::loro_schema::{KEY_TABLE_CELLS, KEY_TABLE_SKELETON};

/// Rejects with an `UnsupportedTableStructure` carrying `msg`.
fn unsupported<T>(msg: impl Into<String>) -> Result<T, MutationError> {
    Err(MutationError::UnsupportedTableStructure(msg.into()))
}

/// Validates that `table` is a simple grid and returns its column count.
fn validate_simple_grid(table: &Table) -> Result<usize, MutationError> {
    let cols = table.col_specs.len();
    if !table.head.rows.is_empty() || !table.foot.rows.is_empty() {
        return unsupported("table has head/foot rows");
    }
    if table.bodies.len() != 1 || !table.bodies[0].head_rows.is_empty() {
        return unsupported("table does not have exactly one simple body");
    }
    if cols == 0 {
        return unsupported("table has no columns");
    }
    for row in &table.bodies[0].body_rows {
        if row.cells.len() != cols {
            return unsupported("ragged table (row width != column count)");
        }
        if row.cells.iter().any(|c| c.row_span != 1 || c.col_span != 1) {
            return unsupported("table has merged (spanning) cells");
        }
    }
    Ok(cols)
}

/// Loads a table block's skeleton `Table`, its live cell list, and column count,
/// validating that it is a simple grid.
fn load_simple_grid(
    loro: &LoroDoc,
    table_index: usize,
) -> Result<(LoroMap, Table, LoroMovableList, usize), MutationError> {
    let (_, block_map, _) = get_block_map_and_list(loro, table_index)?;
    let json = block_map
        .get(KEY_TABLE_SKELETON)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string());
    let Some(json) = json else {
        return unsupported("block is not a native table");
    };
    let table: Table = serde_json::from_str(&json)
        .map_err(|e| MutationError::UnsupportedTableStructure(format!("skeleton parse: {e}")))?;
    let cells_list = block_map
        .get(KEY_TABLE_CELLS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok());
    let Some(cells_list) = cells_list else {
        return unsupported("table has no cell list");
    };
    let cols = validate_simple_grid(&table)?;
    Ok((block_map, table, cells_list, cols))
}

/// Serializes `table` (all cell blocks emptied) back into the block's skeleton.
fn write_skeleton(block_map: &LoroMap, table: &Table) -> Result<(), MutationError> {
    let mut skeleton = table.clone();
    for body in &mut skeleton.bodies {
        for row in body.head_rows.iter_mut().chain(body.body_rows.iter_mut()) {
            for cell in &mut row.cells {
                cell.blocks = Vec::new();
            }
        }
    }
    let json = serde_json::to_string(&skeleton)
        .map_err(|e| MutationError::UnsupportedTableStructure(format!("serialize: {e}")))?;
    block_map.insert(KEY_TABLE_SKELETON, json)?;
    Ok(())
}

/// Inserts a fresh empty cell (one empty paragraph) into the live cell list at
/// flat `index`, matching how the bridge writes a brand-new grid cell.
fn insert_empty_cell(cells_list: &LoroMovableList, index: usize) -> Result<(), MutationError> {
    let cell_blocks = cells_list.insert_container(index, LoroMovableList::new())?;
    let block_map = cell_blocks.insert_container(0, LoroMap::new())?;
    crate::loro_bridge::map_block(&Block::Para(Vec::new()), &block_map)
        .map_err(|e| MutationError::Encode(e.to_string()))?;
    Ok(())
}

/// The `(rows, cols)` of a simple-grid table block, or `None` when it is not a
/// simple grid (or not a table). Lets the editor map a caret's flat cell index
/// to `(row, col)` and bound its row/column actions.
#[must_use]
pub fn table_grid_dims(loro: &LoroDoc, table_index: usize) -> Option<(usize, usize)> {
    let (_, table, _, cols) = load_simple_grid(loro, table_index).ok()?;
    Some((table.bodies[0].body_rows.len(), cols))
}

/// Inserts an empty row at `at_row` (`0..=rows`): `at_row = r` inserts above row
/// `r`, `at_row = rows` appends. Existing rows shift down.
///
/// # Errors
/// [`MutationError::UnsupportedTableStructure`] when the table is not a simple
/// grid or `at_row > rows`.
pub fn insert_table_row(
    loro: &LoroDoc,
    table_index: usize,
    at_row: usize,
) -> Result<(), MutationError> {
    let (block_map, mut table, cells_list, cols) = load_simple_grid(loro, table_index)?;
    let rows = table.bodies[0].body_rows.len();
    if at_row > rows {
        return unsupported(format!("row {at_row} out of range 0..={rows}"));
    }
    let new_row = Row::new((0..cols).map(|_| Cell::simple(Vec::new())).collect());
    table.bodies[0].body_rows.insert(at_row, new_row);
    write_skeleton(&block_map, &table)?;
    let flat = at_row * cols;
    for i in 0..cols {
        insert_empty_cell(&cells_list, flat + i)?;
    }
    Ok(())
}

/// Deletes row `row` (`0..rows`). Refuses to delete the last remaining row.
///
/// # Errors
/// [`MutationError::UnsupportedTableStructure`] when the table is not a simple
/// grid, `row` is out of range, or it is the only row.
pub fn delete_table_row(
    loro: &LoroDoc,
    table_index: usize,
    row: usize,
) -> Result<(), MutationError> {
    let (block_map, mut table, cells_list, cols) = load_simple_grid(loro, table_index)?;
    let rows = table.bodies[0].body_rows.len();
    if row >= rows {
        return unsupported(format!("row {row} out of range 0..{rows}"));
    }
    if rows <= 1 {
        return unsupported("cannot delete the table's only row");
    }
    table.bodies[0].body_rows.remove(row);
    write_skeleton(&block_map, &table)?;
    // Deleting `cols` times at the same flat index removes the whole row: each
    // delete shifts the next cell of the row into that slot.
    let flat = row * cols;
    for _ in 0..cols {
        if flat >= cells_list.len() {
            break;
        }
        cells_list.delete(flat, 1)?;
    }
    Ok(())
}

/// Inserts an empty column at `at_col` (`0..=cols`): `at_col = c` inserts to the
/// left of column `c`, `at_col = cols` appends. The new column is evenly
/// proportioned.
///
/// # Errors
/// [`MutationError::UnsupportedTableStructure`] when the table is not a simple
/// grid or `at_col > cols`.
pub fn insert_table_column(
    loro: &LoroDoc,
    table_index: usize,
    at_col: usize,
) -> Result<(), MutationError> {
    let (block_map, mut table, cells_list, cols) = load_simple_grid(loro, table_index)?;
    if at_col > cols {
        return unsupported(format!("column {at_col} out of range 0..={cols}"));
    }
    let rows = table.bodies[0].body_rows.len();
    table.col_specs.insert(at_col, ColSpec::proportional(1.0));
    for row in &mut table.bodies[0].body_rows {
        row.cells.insert(at_col, Cell::simple(Vec::new()));
    }
    write_skeleton(&block_map, &table)?;
    // Insert one cell per row at flat `r*cols + at_col`, last row first so a
    // higher-index insertion never shifts a lower row's target position.
    for r in (0..rows).rev() {
        insert_empty_cell(&cells_list, r * cols + at_col)?;
    }
    Ok(())
}

/// Deletes column `col` (`0..cols`). Refuses to delete the last remaining column.
///
/// # Errors
/// [`MutationError::UnsupportedTableStructure`] when the table is not a simple
/// grid, `col` is out of range, or it is the only column.
pub fn delete_table_column(
    loro: &LoroDoc,
    table_index: usize,
    col: usize,
) -> Result<(), MutationError> {
    let (block_map, mut table, cells_list, cols) = load_simple_grid(loro, table_index)?;
    if col >= cols {
        return unsupported(format!("column {col} out of range 0..{cols}"));
    }
    if cols <= 1 {
        return unsupported("cannot delete the table's only column");
    }
    let rows = table.bodies[0].body_rows.len();
    table.col_specs.remove(col);
    for row in &mut table.bodies[0].body_rows {
        row.cells.remove(col);
    }
    write_skeleton(&block_map, &table)?;
    // Delete one cell per row at flat `r*cols + col`, last row first so a
    // deletion never shifts a lower row's target position.
    for r in (0..rows).rev() {
        let idx = r * cols + col;
        if idx < cells_list.len() {
            cells_list.delete(idx, 1)?;
        }
    }
    Ok(())
}
