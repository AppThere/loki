// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Block-level structural mutations: split, merge, and insert.

#[cfg(feature = "serde")]
use crate::content::block::Block;
use loro::{LoroDoc, LoroMap, LoroMovableList, LoroText};

use crate::loro_schema::{
    KEY_CONTENT, KEY_DIRECT_CHAR_PROPS, KEY_HEADING_LEVEL, KEY_PARA_PROPS, KEY_TYPE,
};

use super::nested::resolve_block_list;
use super::{
    BlockPath, MutationError, copy_map_primitive_values, get_block_map_and_list,
    resolve_section_blocks,
};

/// Splits the block at `block_index` at `byte_offset`, inserting a new block
/// immediately after it.
///
/// After the split:
/// - Block `block_index` contains `text[..byte_offset]`.
/// - Block `block_index + 1` contains `text[byte_offset..]`.
/// - The new block inherits `KEY_TYPE`, `KEY_PARA_PROPS`, and
///   `KEY_DIRECT_CHAR_PROPS` from the source block.
///
/// `byte_offset == 0` produces an empty first block followed by the full text.
/// `byte_offset == text.len()` produces the full text followed by an empty block.
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] — `block_index` is out of range.
/// - [`MutationError::TextNotFound`] — the block has no `LoroText` content.
/// - [`MutationError::InvalidByteOffset`] — `byte_offset` exceeds the text
///   length or falls on a non-character boundary.
/// - [`MutationError::Loro`] — an underlying Loro error.
pub fn split_block(
    loro: &LoroDoc,
    block_index: usize,
    byte_offset: usize,
) -> Result<(), MutationError> {
    let (blocks_list, block_map, local) = get_block_map_and_list(loro, block_index)?;
    split_block_in_list(&blocks_list, &block_map, local, byte_offset, block_index)
}

/// Path-aware [`split_block`]: splits the block addressed by `path` at
/// `byte_offset`, inserting the tail as a new sibling immediately after it
/// *within the same container* (top-level section, table cell, or note body).
///
/// The new block inherits type and props exactly as for [`split_block`]; the
/// difference is only *where* the block list lives — the leaf step's container
/// rather than the section. The cursor's next block is `path` with the leaf
/// block index incremented by one.
///
/// # Errors
///
/// As for [`split_block`], plus [`MutationError::InvalidBlockPath`] when a
/// descent step of `path` is invalid.
pub fn split_block_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
) -> Result<(), MutationError> {
    let (blocks_list, local) = resolve_block_list(loro, path)?;
    let block_map = blocks_list
        .get(local)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .ok_or_else(|| MutationError::InvalidBlockPath(format!("no block {local} to split")))?;
    split_block_in_list(&blocks_list, &block_map, local, byte_offset, path.root)
}

/// Core split: splits `block_map` (the block at `local` in `blocks_list`) at
/// `byte_offset`, inserting the tail as a new block at `local + 1` in the same
/// list. `block_index` is used only for error reporting.
fn split_block_in_list(
    blocks_list: &LoroMovableList,
    block_map: &LoroMap,
    local: usize,
    byte_offset: usize,
    block_index: usize,
) -> Result<(), MutationError> {
    // Get the LoroText for the source block.
    let text_container = block_map
        .get(KEY_CONTENT)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .ok_or(MutationError::TextNotFound(block_index))?;

    let full_text = text_container.to_string();

    // Validate that byte_offset is on a UTF-8 character boundary.
    if byte_offset > full_text.len() || !full_text.is_char_boundary(byte_offset) {
        return Err(MutationError::InvalidByteOffset {
            offset: byte_offset,
        });
    }

    let tail = full_text[byte_offset..].to_string();

    // Remove the tail from the source block first.
    if !tail.is_empty() {
        text_container.delete_utf8(byte_offset, tail.len())?;
    }

    // Insert a new LoroMap right after the source block, within its section's
    // blocks list (`local` is the source block's index in that section).
    let new_map = blocks_list.insert_container(local + 1, LoroMap::new())?;

    // Copy KEY_TYPE (required; default to empty string if absent).
    let block_type = block_map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();
    new_map.insert(KEY_TYPE, block_type.as_str())?;

    // Copy KEY_PARA_PROPS if present on the source block.
    if let Some(src) = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        let dst = new_map.insert_container(KEY_PARA_PROPS, LoroMap::new())?;
        copy_map_primitive_values(&src, &dst)?;
    }

    // Copy KEY_DIRECT_CHAR_PROPS if present on the source block.
    if let Some(src) = block_map
        .get(KEY_DIRECT_CHAR_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        let dst = new_map.insert_container(KEY_DIRECT_CHAR_PROPS, LoroMap::new())?;
        copy_map_primitive_values(&src, &dst)?;
    }

    // Copy heading level if present (stored as a plain integer, not in a sub-map).
    if let Some(level_val) = block_map
        .get(KEY_HEADING_LEVEL)
        .and_then(|v| v.into_value().ok())
    {
        new_map.insert(KEY_HEADING_LEVEL, level_val)?;
    }

    // Create the content LoroText with the tail.
    let new_text = new_map.insert_container(KEY_CONTENT, LoroText::new())?;
    if !tail.is_empty() {
        new_text.insert_utf8(0, &tail)?;
    }

    Ok(())
}

/// Merges block `block_index` into block `block_index - 1`.
///
/// After the merge:
/// - Block `block_index - 1` contains its original text followed by all of
///   block `block_index`'s text.
/// - Block `block_index` is removed from the blocks list.
///
/// Returns the byte offset within the merged block where the two texts
/// join — i.e. the former length (in UTF-8 bytes) of block `block_index - 1`.
/// The caller should position the cursor at this offset after the merge.
///
/// # Errors
///
/// - [`MutationError::NoPreviousBlock`] — `block_index` is 0 (no predecessor).
/// - [`MutationError::CrossSectionMerge`] — `block_index` is the first block of
///   its section, so its predecessor lives in an earlier section. Merging across
///   a section break (which would remove the break) is not supported.
/// - [`MutationError::BlockIndexOutOfRange`] — either `block_index` or
///   `block_index - 1` is out of range (e.g. the document is empty).
/// - [`MutationError::TextNotFound`] — one of the blocks has no `LoroText`.
/// - [`MutationError::Loro`] — an underlying Loro error.
pub fn merge_block(loro: &LoroDoc, block_index: usize) -> Result<usize, MutationError> {
    if block_index == 0 {
        return Err(MutationError::NoPreviousBlock);
    }

    // Resolve the current block to its section's list and its index within that
    // section. A `local` of 0 means the previous block is the last block of an
    // earlier section, so this would merge across a section break.
    let (blocks_list, local) = resolve_section_blocks(loro, block_index)?;
    if local == 0 {
        return Err(MutationError::CrossSectionMerge);
    }
    merge_block_in_list(&blocks_list, local, block_index)
}

/// Path-aware [`merge_block`]: merges the block addressed by `path` into its
/// previous sibling *within the same container* (top-level section, table cell,
/// or note body), returning the join offset (the prior UTF-8 length of the
/// previous block). The cursor's merged position is `path` with the leaf block
/// index decremented by one and that join offset.
///
/// # Errors
///
/// - [`MutationError::NoPreviousBlock`] — the addressed block is the first of
///   its container, so there is no sibling to merge into (a container boundary
///   is never crossed).
/// - [`MutationError::InvalidBlockPath`] — a descent step of `path` is invalid.
/// - [`MutationError::TextNotFound`] / [`MutationError::Loro`] — as for
///   [`merge_block`].
pub fn merge_block_at(loro: &LoroDoc, path: &BlockPath) -> Result<usize, MutationError> {
    let (blocks_list, local) = resolve_block_list(loro, path)?;
    if local == 0 {
        return Err(MutationError::NoPreviousBlock);
    }
    merge_block_in_list(&blocks_list, local, path.root)
}

/// Core merge: appends block `local`'s text into block `local - 1` within
/// `blocks_list`, then removes block `local`. Returns the join offset (the prior
/// UTF-8 length of block `local - 1`). Callers must ensure `local >= 1`.
/// `block_index` is used only for error reporting.
fn merge_block_in_list(
    blocks_list: &LoroMovableList,
    local: usize,
    block_index: usize,
) -> Result<usize, MutationError> {
    // Read the current block's text before mutating.
    let tail = list_block_text(blocks_list, local, block_index)?.to_string();

    // Append it to the previous block (`local - 1`).
    let prev_text = list_block_text(blocks_list, local - 1, block_index)?;
    let merged_offset = prev_text.len_utf8();
    if !tail.is_empty() {
        prev_text.insert_utf8(merged_offset, &tail)?;
    }

    // Remove the now-merged block from its list.
    if local >= blocks_list.len() {
        return Err(MutationError::BlockIndexOutOfRange(block_index));
    }
    blocks_list.delete(local, 1)?;

    Ok(merged_offset)
}

/// Navigates to the `LoroText` content of block `local` in `blocks_list`.
/// `block_index` is used only for the [`MutationError::TextNotFound`] payload.
fn list_block_text(
    blocks_list: &LoroMovableList,
    local: usize,
    block_index: usize,
) -> Result<LoroText, MutationError> {
    blocks_list
        .get(local)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|m| m.get(KEY_CONTENT))
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .ok_or(MutationError::TextNotFound(block_index))
}

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
