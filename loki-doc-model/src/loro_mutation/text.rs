// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Character-level text mutations for individual blocks.

use loro::LoroDoc;

use super::{get_loro_text_for_block, MutationError};

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
