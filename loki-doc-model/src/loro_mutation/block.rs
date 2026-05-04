// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Block-level structural mutations: split and merge.

use loro::{LoroDoc, LoroMap, LoroText};

use crate::loro_schema::{KEY_CONTENT, KEY_DIRECT_CHAR_PROPS, KEY_PARA_PROPS, KEY_TYPE};

use super::{
    copy_map_primitive_values, get_block_map_and_list, get_loro_text_for_block, MutationError,
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
    let (blocks_list, block_map) = get_block_map_and_list(loro, block_index)?;

    // Get the LoroText for the source block.
    let text_container = block_map
        .get(KEY_CONTENT)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .ok_or(MutationError::TextNotFound(block_index))?;

    let full_text = text_container.to_string();

    // Validate that byte_offset is on a UTF-8 character boundary.
    if byte_offset > full_text.len() || !full_text.is_char_boundary(byte_offset) {
        return Err(MutationError::InvalidByteOffset { offset: byte_offset });
    }

    let tail = full_text[byte_offset..].to_string();

    // Remove the tail from the source block first.
    if !tail.is_empty() {
        text_container.delete_utf8(byte_offset, tail.len())?;
    }

    // Insert a new LoroMap at block_index + 1 in the blocks list.
    let new_map = blocks_list.insert_container(block_index + 1, LoroMap::new())?;

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
/// - [`MutationError::BlockIndexOutOfRange`] — either `block_index` or
///   `block_index - 1` is out of range (e.g. the document is empty).
/// - [`MutationError::TextNotFound`] — one of the blocks has no `LoroText`.
/// - [`MutationError::Loro`] — an underlying Loro error.
pub fn merge_block(loro: &LoroDoc, block_index: usize) -> Result<usize, MutationError> {
    if block_index == 0 {
        return Err(MutationError::NoPreviousBlock);
    }
    let prev_index = block_index - 1;

    // Validate block_index and read its text before mutating.
    let curr_text_container = get_loro_text_for_block(loro, block_index)?;
    let tail = curr_text_container.to_string();

    // Validate prev_index and get its LoroText.
    let prev_text = get_loro_text_for_block(loro, prev_index)?;
    let merged_offset = prev_text.len_utf8();

    // Append block N's text to block N-1.
    if !tail.is_empty() {
        prev_text.insert_utf8(merged_offset, &tail)?;
    }

    // Remove block N. The blocks list is navigated fresh here; prev_index
    // was already validated above so the section path is guaranteed to exist.
    let (blocks_list, _) = get_block_map_and_list(loro, prev_index)?;
    // Re-check block_index against the live list length.
    if block_index >= blocks_list.len() {
        return Err(MutationError::BlockIndexOutOfRange(block_index));
    }
    blocks_list.delete(block_index, 1)?;

    Ok(merged_offset)
}
