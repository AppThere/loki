// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! CRDT mutation layer for the Loki document editor.
//!
//! Provides typed helpers for inserting/deleting text and splitting/merging
//! blocks within a [`LoroDoc`], using the container path established by
//! [`crate::loro_bridge::document_to_loro`].
//!
//! # Submodules
//!
//! - [`text`] — character-level mutations: [`insert_text`], [`delete_text`],
//!   [`get_block_text`]
//! - [`block`] — block-level mutations: [`split_block`], [`merge_block`]
//!
//! # Container Path
//!
//! Text for block N in section 0 lives at:
//! ```text
//! sections[0].blocks[N]["content"]  (LoroText)
//! ```
//!
//! # Byte Offsets
//!
//! All `byte_offset` and `len` parameters are **UTF-8 byte positions**.

mod block;
mod text;

pub use self::block::{merge_block, split_block};
pub use self::text::{delete_text, get_block_text, insert_text};

use loro::{LoroDoc, LoroMap, LoroMovableList, LoroText};

use crate::loro_schema::{KEY_BLOCKS, KEY_CONTENT, KEY_SECTIONS};

/// Errors that can occur when mutating a [`LoroDoc`] block's text or structure.
#[derive(Debug, thiserror::Error)]
pub enum MutationError {
    /// The requested block index is out of range for section 0.
    #[error("Block index {0} out of range")]
    BlockIndexOutOfRange(usize),
    /// No `LoroText` container was found under the block's `"content"` key.
    #[error("LoroText not found for block {0}")]
    TextNotFound(usize),
    /// An error returned by the underlying Loro library.
    #[error("Loro error: {0}")]
    Loro(String),
    /// `byte_offset` is out of range or not on a UTF-8 character boundary.
    #[error("Invalid byte offset {offset} for block split")]
    InvalidByteOffset { offset: usize },
    /// `merge_block` was called on block 0, which has no predecessor.
    #[error("Cannot merge: no block before block 0")]
    NoPreviousBlock,
}

impl From<loro::LoroError> for MutationError {
    fn from(e: loro::LoroError) -> Self {
        MutationError::Loro(e.to_string())
    }
}

// ── Shared internal helpers ───────────────────────────────────────────────────

/// Navigate to the `LoroText` container for `block_index` in section 0.
pub(crate) fn get_loro_text_for_block(
    loro: &LoroDoc,
    block_index: usize,
) -> Result<LoroText, MutationError> {
    let sections_list = loro.get_list(KEY_SECTIONS);
    let sec_val = sections_list
        .get(0)
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    let sec_map = sec_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;

    let blocks_val = sec_map
        .get(KEY_BLOCKS)
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    let blocks_list = blocks_val
        .into_container()
        .ok()
        .and_then(|c| c.into_movable_list().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;

    if block_index >= blocks_list.len() {
        return Err(MutationError::BlockIndexOutOfRange(block_index));
    }

    let block_val = blocks_list
        .get(block_index)
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    let block_map = block_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;

    let content_val = block_map
        .get(KEY_CONTENT)
        .ok_or(MutationError::TextNotFound(block_index))?;
    let text = content_val
        .into_container()
        .ok()
        .and_then(|c| c.into_text().ok())
        .ok_or(MutationError::TextNotFound(block_index))?;

    Ok(text)
}

/// Navigate to the `LoroMovableList` of blocks and the `LoroMap` for
/// `block_index` in section 0.
///
/// Returns both so callers can insert/delete from the list after reading
/// the block map.
pub(crate) fn get_block_map_and_list(
    loro: &LoroDoc,
    block_index: usize,
) -> Result<(LoroMovableList, LoroMap), MutationError> {
    let sections_list = loro.get_list(KEY_SECTIONS);
    let sec_val = sections_list
        .get(0)
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    let sec_map = sec_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;

    let blocks_val = sec_map
        .get(KEY_BLOCKS)
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    let blocks_list = blocks_val
        .into_container()
        .ok()
        .and_then(|c| c.into_movable_list().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;

    if block_index >= blocks_list.len() {
        return Err(MutationError::BlockIndexOutOfRange(block_index));
    }

    let block_val = blocks_list
        .get(block_index)
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    let block_map = block_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;

    Ok((blocks_list, block_map))
}

/// Copies primitive (non-container) key-value pairs from `src` to `dst`.
///
/// Container-typed values (nested LoroMaps, etc.) are silently skipped since
/// `KEY_PARA_PROPS` and `KEY_DIRECT_CHAR_PROPS` only contain primitive entries.
pub(crate) fn copy_map_primitive_values(
    src: &LoroMap,
    dst: &LoroMap,
) -> Result<(), MutationError> {
    let mut err: Option<MutationError> = None;
    src.for_each(|k, v| {
        if err.is_some() {
            return;
        }
        if let Ok(loro_val) = v.into_value()
            && let Err(e) = dst.insert(k, loro_val)
        {
            err = Some(MutationError::Loro(e.to_string()));
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(())
}
