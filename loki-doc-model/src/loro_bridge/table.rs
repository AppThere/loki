// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Native CRDT mapping for `Block::Table`.
//!
//! A table is stored as two parts so it round-trips losslessly *and* its cell
//! text becomes live, mergeable CRDT state rather than one opaque JSON blob:
//!
//! - **Skeleton** ([`KEY_TABLE_SKELETON`]): a `serde`-JSON snapshot of the whole
//!   [`Table`] with every cell's blocks emptied. This carries the structural
//!   metadata — column specs/widths, section/row layout, row/column spans, cell
//!   and row properties, borders, caption, and node attributes.
//! - **Cell contents** ([`KEY_TABLE_CELLS`]): a movable list with one entry per
//!   cell, each a movable list of that cell's blocks written through the shared
//!   block path ([`map_blocks_to_list`]). Cell text therefore lives in real
//!   `LoroText` containers, so concurrent edits to different cells merge.
//!
//! The two parts are joined by a single deterministic traversal order
//! ([`cells_in_order`] / [`cells_in_order_mut`]) — the i-th cell-content list
//! fills the i-th cell of the skeleton. Both functions MUST visit cells in the
//! same order; keep them in sync.
//!
//! Structural edits (adding a row, changing a span) still rewrite the skeleton
//! blob and so do not merge at that granularity; a fully structural CRDT
//! mapping is future TODO(loro-bridge) work. Without the `serde` feature the
//! table has no skeleton format, so it falls back to the opaque path.

use super::BridgeError;
use crate::content::block::Block;
use crate::content::table::core::Table;
use crate::content::table::row::Cell;
use crate::loro_schema::{BLOCK_TYPE_TABLE, KEY_TABLE_CELLS, KEY_TABLE_SKELETON, KEY_TYPE};
use loro::{LoroMap, LoroMovableList};

/// Visits every cell once, in head → bodies (head rows then body rows) → foot,
/// row-major order. The write/read paths share this order to pair each cell
/// with its content list.
fn cells_in_order(table: &Table) -> impl Iterator<Item = &Cell> {
    let head = table.head.rows.iter();
    let bodies = table
        .bodies
        .iter()
        .flat_map(|b| b.head_rows.iter().chain(b.body_rows.iter()));
    let foot = table.foot.rows.iter();
    head.chain(bodies)
        .chain(foot)
        .flat_map(|row| row.cells.iter())
}

/// Mutable twin of [`cells_in_order`] — MUST visit cells in the identical order.
fn cells_in_order_mut(table: &mut Table) -> impl Iterator<Item = &mut Cell> {
    let head = table.head.rows.iter_mut();
    let bodies = table
        .bodies
        .iter_mut()
        .flat_map(|b| b.head_rows.iter_mut().chain(b.body_rows.iter_mut()));
    let foot = table.foot.rows.iter_mut();
    head.chain(bodies)
        .chain(foot)
        .flat_map(|row| row.cells.iter_mut())
}

/// Writes `table` into `map` as a native [`BLOCK_TYPE_TABLE`] block.
#[cfg(feature = "serde")]
pub(super) fn write_table(table: &Table, map: &LoroMap) -> Result<(), BridgeError> {
    map.insert(KEY_TYPE, BLOCK_TYPE_TABLE)?;

    // Skeleton: the whole table with cell blocks stripped out.
    let mut skeleton = table.clone();
    for cell in cells_in_order_mut(&mut skeleton) {
        cell.blocks = Vec::new();
    }
    match serde_json::to_string(&skeleton) {
        Ok(json) => {
            map.insert(KEY_TABLE_SKELETON, json)?;
        }
        Err(err) => {
            // Unreachable in practice: every Table field derives Serialize.
            tracing::warn!("loro bridge: failed to snapshot table skeleton: {err}");
        }
    }

    // Live cell contents, one nested block list per cell (shared block path).
    let cells_list = map.insert_container(KEY_TABLE_CELLS, LoroMovableList::new())?;
    for (i, cell) in cells_in_order(table).enumerate() {
        let cell_blocks = cells_list.insert_container(i, LoroMovableList::new())?;
        super::write::map_blocks_to_list(&cell.blocks, &cell_blocks)?;
    }
    Ok(())
}

/// Reads a native [`BLOCK_TYPE_TABLE`] block back into a [`Block::Table`].
///
/// Falls back to [`Block::HorizontalRule`] when the skeleton is missing or
/// unparseable (e.g. a legacy stub, or a table written without `serde`).
pub(super) fn read_table(map: &LoroMap) -> Block {
    match read_table_inner(map) {
        Some(table) => Block::Table(Box::new(table)),
        None => {
            tracing::warn!("loro bridge: unreadable native table; dropping to rule");
            Block::HorizontalRule
        }
    }
}

#[cfg(feature = "serde")]
fn read_table_inner(map: &LoroMap) -> Option<Table> {
    let json = map
        .get(KEY_TABLE_SKELETON)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())?;
    let mut table: Table = serde_json::from_str(&json).ok()?;

    let cells_list = map
        .get(KEY_TABLE_CELLS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok());
    if let Some(cells_list) = cells_list {
        for (i, cell) in cells_in_order_mut(&mut table).enumerate() {
            if let Some(list) = cells_list
                .get(i)
                .and_then(|v| v.into_container().ok())
                .and_then(|c| c.into_movable_list().ok())
            {
                cell.blocks = super::read::reconstruct_blocks_from_list(&list);
            }
        }
    }
    Some(table)
}

#[cfg(not(feature = "serde"))]
fn read_table_inner(_map: &LoroMap) -> Option<Table> {
    None
}
