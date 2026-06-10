// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Character-level text mutations for individual blocks.

use loro::{LoroDoc, LoroValue, TextDelta};

use super::{MutationError, get_loro_text_for_block};

/// Inserts `text` at UTF-8 `byte_offset` into the `LoroText` for the block
/// at `block_index` (in section 0).
///
/// # Errors
///
/// Returns [`MutationError::BlockIndexOutOfRange`] when `block_index` is
/// out of range, [`MutationError::TextNotFound`] when the block has no
/// `LoroText` content (e.g. a stub table block), or [`MutationError::Loro`]
/// for any Loro internal error.
pub fn insert_text(
    loro: &LoroDoc,
    block_index: usize,
    byte_offset: usize,
    text: &str,
) -> Result<(), MutationError> {
    let loro_text = get_loro_text_for_block(loro, block_index)?;
    loro_text.insert_utf8(byte_offset, text)?;
    Ok(())
}

/// Deletes `len` UTF-8 bytes starting at `byte_offset` from the `LoroText`
/// for the block at `block_index` (in section 0).
///
/// A `len` of `0` is a no-op (returns `Ok` immediately without touching Loro).
///
/// # Errors
///
/// Returns [`MutationError::BlockIndexOutOfRange`] when `block_index` is
/// out of range, [`MutationError::TextNotFound`] when the block has no
/// `LoroText` content, or [`MutationError::Loro`] for any Loro internal error.
pub fn delete_text(
    loro: &LoroDoc,
    block_index: usize,
    byte_offset: usize,
    len: usize,
) -> Result<(), MutationError> {
    if len == 0 {
        return Ok(());
    }
    let loro_text = get_loro_text_for_block(loro, block_index)?;
    loro_text.delete_utf8(byte_offset, len)?;
    Ok(())
}

/// Returns the current plain-text content of the block at `block_index`
/// (section 0) as a `String`.
///
/// Returns an empty string when `block_index` is out of range or the block
/// has no `LoroText` (rather than panicking).
pub fn get_block_text(loro: &LoroDoc, block_index: usize) -> String {
    get_loro_text_for_block(loro, block_index)
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// Applies a rich-text mark (character-level formatting) to a UTF-8 byte
/// range within a block's text content.
///
/// Pass `LoroValue::Null` as `mark_value` to remove a mark (clear formatting).
/// Pass `LoroValue::Bool(true)` for boolean marks such as bold or italic.
///
/// # Errors
///
/// Returns [`MutationError::BlockIndexOutOfRange`] when `block_index` is out
/// of range, or [`MutationError::Loro`] for any Loro internal error.
///
/// A `byte_start >= byte_end` range is a no-op (returns `Ok` immediately).
pub fn mark_text(
    loro: &LoroDoc,
    block_index: usize,
    byte_start: usize,
    byte_end: usize,
    mark_key: &str,
    mark_value: LoroValue,
) -> Result<(), MutationError> {
    if byte_start >= byte_end {
        return Ok(());
    }
    let text = get_loro_text_for_block(loro, block_index)?;
    // COMPAT(loro): mark_utf8 uses UTF-8 byte offsets, matching CursorState's
    // coordinate space. The non-suffixed mark() uses unicode character positions.
    text.mark_utf8(byte_start..byte_end, mark_key, mark_value)
        .map_err(MutationError::from)
}

/// Returns the value of a named mark at a UTF-8 byte offset within a block,
/// or `None` if the mark is not set at that position.
///
/// Used to determine current toggle state for ribbon buttons and keyboard
/// shortcuts (e.g. whether Bold is active at the cursor).
///
/// # Errors
///
/// Returns [`MutationError::BlockIndexOutOfRange`] when `block_index` is out
/// of range, or [`MutationError::TextNotFound`] when the block has no text.
pub fn get_mark_at(
    loro: &LoroDoc,
    block_index: usize,
    byte_offset: usize,
    mark_key: &str,
) -> Result<Option<LoroValue>, MutationError> {
    let text = get_loro_text_for_block(loro, block_index)?;
    // Walk richtext delta spans accumulating byte position until we find the
    // span that contains byte_offset, then extract the requested mark.
    // to_delta() returns only TextDelta::Insert spans (no Retain/Delete).
    let mut byte_pos = 0usize;
    for delta in text.to_delta() {
        if let TextDelta::Insert { insert, attributes } = delta {
            let span_bytes = insert.len();
            if byte_offset < byte_pos + span_bytes {
                return Ok(attributes.and_then(|attrs| attrs.get(mark_key).cloned()));
            }
            byte_pos += span_bytes;
        }
    }
    Ok(None)
}
