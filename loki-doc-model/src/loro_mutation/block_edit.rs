// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Whole-block insert and delete mutations.
//!
//! Split out of [`super::block`] (which handles split/merge) to keep both files
//! under the 300-line ceiling.

#[cfg(feature = "serde")]
use crate::content::block::Block;
use loro::{LoroDoc, LoroMap};

use super::{MutationError, get_block_map_and_list};

/// Inserts `block` as a new top-level block immediately after the block at
/// `block_index` (within the same section), returning the new block's
/// document-global index (`block_index + 1`).
///
/// The block is written with the bridge's own schema, so it round-trips through
/// `loro_to_document` and (for a `Block::Table`) its cells become live editable
/// containers reachable via a `BlockPath`. Used by the editor's Insert → Table
/// control. Nesting (inserting a block inside a cell/note) is not addressed
/// here — the cursor's root block is used.
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] — `block_index` is out of range.
/// - [`MutationError::Encode`] — the bridge could not serialize `block`.
/// - [`MutationError::Loro`] — an underlying Loro error.
#[cfg(feature = "serde")]
pub fn insert_block_after(
    loro: &LoroDoc,
    block_index: usize,
    block: &Block,
) -> Result<usize, MutationError> {
    let (blocks_list, _block_map, local) = get_block_map_and_list(loro, block_index)?;
    let new_map = blocks_list.insert_container(local + 1, LoroMap::new())?;
    crate::loro_bridge::map_block(block, &new_map)
        .map_err(|e| MutationError::Encode(e.to_string()))?;
    Ok(block_index + 1)
}

/// Removes the top-level block at `block_index` (within its section) — e.g. the
/// editor's contextual Table tab "Delete Table" action removing the table block
/// the caret sits in.
///
/// This deletes exactly one block and does **not** guard against emptying a
/// section: the caller must ensure the document keeps at least one editable
/// block (the editor disables Delete Table when the table is the sole block).
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] if `block_index` is out of range.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn delete_block(loro: &LoroDoc, block_index: usize) -> Result<(), MutationError> {
    let (blocks_list, _block_map, local) = get_block_map_and_list(loro, block_index)?;
    if local >= blocks_list.len() {
        return Err(MutationError::BlockIndexOutOfRange(block_index));
    }
    blocks_list.delete(local, 1)?;
    Ok(())
}
