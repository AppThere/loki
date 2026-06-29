// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Addressing for content nested inside container blocks (currently table
//! cells), and text mutations against it.
//!
//! The flat mutation API addresses a block by a document-global index into the
//! section block lists. That cannot reach a paragraph *inside a table cell*,
//! whose `LoroText` lives under the table block's [`KEY_TABLE_CELLS`] container
//! (see `loro_bridge::table`). A [`BlockPath`] names such a target: a root
//! block plus zero or more [`CellStep`] descents — each selecting a cell (in the
//! bridge's flat head → bodies → foot order) and a block within it. The path is
//! recursive, so a table nested in a cell is reachable too.
//!
//! Mutating the live cell `LoroText` round-trips: `loro_bridge::table` rebuilds
//! each cell's blocks from these same containers on read.

use loro::{LoroDoc, LoroMap, LoroText, LoroValue};

use super::{MutationError, get_block_map_and_list};
use crate::loro_schema::{KEY_CONTENT, KEY_TABLE_CELLS};

/// One descent into a table block: the `cell`-th cell (in the bridge's flat
/// head → bodies → foot, row-major order) and the `block`-th block within it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellStep {
    /// Flat cell index within the table's [`KEY_TABLE_CELLS`] list.
    pub cell: usize,
    /// Block index within that cell's content.
    pub block: usize,
}

/// A path to a block, either top-level or nested inside table cell(s).
///
/// `root` is a document-global block index (the same space the flat API and the
/// cursor use); `steps` descends through table cells. An empty `steps` resolves
/// exactly like the flat API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockPath {
    /// Document-global index of the root block.
    pub root: usize,
    /// Successive cell descents from the root.
    pub steps: Vec<CellStep>,
}

impl BlockPath {
    /// A top-level block (no nesting) — equivalent to the flat API.
    #[must_use]
    pub fn block(root: usize) -> Self {
        Self {
            root,
            steps: Vec::new(),
        }
    }

    /// A block at `block` inside the `cell`-th cell of the table at `root`.
    #[must_use]
    pub fn in_cell(root: usize, cell: usize, block: usize) -> Self {
        Self {
            root,
            steps: vec![CellStep { cell, block }],
        }
    }
}

/// Descends one [`CellStep`] from a table block's map to a nested block's map.
fn descend(table_map: &LoroMap, step: CellStep) -> Result<LoroMap, MutationError> {
    let cells = table_map
        .get(KEY_TABLE_CELLS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
        .ok_or_else(|| MutationError::InvalidBlockPath("block is not a table".to_string()))?;
    let cell_list = cells
        .get(step.cell)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
        .ok_or_else(|| MutationError::InvalidBlockPath(format!("no cell {}", step.cell)))?;
    cell_list
        .get(step.block)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .ok_or_else(|| {
            MutationError::InvalidBlockPath(format!(
                "no block {} in cell {}",
                step.block, step.cell
            ))
        })
}

/// Resolves `path` to the block's `LoroMap`.
fn resolve_block_map(loro: &LoroDoc, path: &BlockPath) -> Result<LoroMap, MutationError> {
    let (_, mut block_map, _) = get_block_map_and_list(loro, path.root)?;
    for step in &path.steps {
        block_map = descend(&block_map, *step)?;
    }
    Ok(block_map)
}

/// Resolves `path` to the `LoroText` content container of the addressed block.
fn text_for_path(loro: &LoroDoc, path: &BlockPath) -> Result<LoroText, MutationError> {
    resolve_block_map(loro, path)?
        .get(KEY_CONTENT)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .ok_or(MutationError::TextNotFound(path.root))
}

/// Inserts `text` at UTF-8 `byte_offset` into the block addressed by `path`.
///
/// # Errors
///
/// [`MutationError::InvalidBlockPath`] when a descent step is invalid,
/// [`MutationError::TextNotFound`] when the target has no text, or
/// [`MutationError::Loro`] for an internal Loro error.
pub fn insert_text_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    text: &str,
) -> Result<(), MutationError> {
    text_for_path(loro, path)?.insert_utf8(byte_offset, text)?;
    Ok(())
}

/// Deletes `len` UTF-8 bytes at `byte_offset` from the block addressed by
/// `path`. A `len` of `0` is a no-op.
pub fn delete_text_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    len: usize,
) -> Result<(), MutationError> {
    if len == 0 {
        return Ok(());
    }
    text_for_path(loro, path)?.delete_utf8(byte_offset, len)?;
    Ok(())
}

/// Applies a mark over a UTF-8 byte range in the block addressed by `path`.
/// A `byte_start >= byte_end` range is a no-op.
pub fn mark_text_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_start: usize,
    byte_end: usize,
    mark_key: &str,
    mark_value: LoroValue,
) -> Result<(), MutationError> {
    if byte_start >= byte_end {
        return Ok(());
    }
    text_for_path(loro, path)?
        .mark_utf8(byte_start..byte_end, mark_key, mark_value)
        .map_err(MutationError::from)
}

/// Returns the plain text of the block addressed by `path` (empty when the path
/// does not resolve to a text block).
#[must_use]
pub fn get_block_text_at(loro: &LoroDoc, path: &BlockPath) -> String {
    text_for_path(loro, path)
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// Returns the value of `mark_key` at UTF-8 `byte_offset` in the block
/// addressed by `path`, or `None` if unset there.
pub fn get_mark_at_path(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    mark_key: &str,
) -> Result<Option<LoroValue>, MutationError> {
    let text = text_for_path(loro, path)?;
    let mut byte_pos = 0usize;
    for delta in text.to_delta() {
        if let loro::TextDelta::Insert { insert, attributes } = delta {
            let span_bytes = insert.len();
            if byte_offset < byte_pos + span_bytes {
                return Ok(attributes.and_then(|attrs| attrs.get(mark_key).cloned()));
            }
            byte_pos += span_bytes;
        }
    }
    Ok(None)
}
