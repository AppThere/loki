// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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
//! # Block addressing
//!
//! Block indices are **document-global**: section 0's blocks occupy the first
//! `sections[0].blocks.len()` indices, section 1's the next, and so on — the same
//! flat index space the layout assigns to paragraphs and the editor's cursor
//! uses. [`resolve_section_blocks`] maps a global index to the containing
//! section's blocks list and the block's local index, i.e.
//! `sections[S].blocks[local]["content"]` (a `LoroText`).
//!
//! # Byte Offsets
//!
//! All `byte_offset` and `len` parameters are **UTF-8 byte positions**.

mod block;
mod nested;
#[cfg(feature = "serde")]
mod objects;
mod style;
mod text;

#[cfg(feature = "serde")]
pub use self::block::insert_block_after;
pub use self::block::{merge_block, merge_block_at, split_block, split_block_at};
pub use self::nested::{
    BlockPath, PathStep, delete_text_at, get_block_text_at, get_mark_at_path, insert_text_at,
    mark_text_at,
};
#[cfg(feature = "serde")]
pub use self::objects::{insert_inline_image_at, insert_inline_note_at};
pub use self::style::{
    get_block_alignment, get_block_style_name, set_block_alignment, set_block_style,
    set_block_type_heading, set_block_type_para,
};
#[cfg(feature = "serde")]
pub use self::text::insert_inline_image;
pub use self::text::{
    delete_text, get_block_text, get_mark_at, insert_text, mark_text, replace_text,
};

use loro::{LoroDoc, LoroList, LoroMap, LoroMovableList, LoroText};

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
    /// Failed to serialize structured inline content (e.g. an image) to the
    /// JSON snapshot carried by an inline-object mark.
    #[error("Encoding error: {0}")]
    Encode(String),
    /// `byte_offset` is out of range or not on a UTF-8 character boundary.
    #[error("Invalid byte offset {offset} for block split")]
    InvalidByteOffset { offset: usize },
    /// `merge_block` was called on block 0, which has no predecessor.
    #[error("Cannot merge: no block before block 0")]
    NoPreviousBlock,
    /// `merge_block` would merge across a section break — the previous block
    /// lives in an earlier section. Merging across a section boundary (which
    /// would remove the break) is not supported.
    #[error("Cannot merge across a section break")]
    CrossSectionMerge,
    /// A [`nested::BlockPath`] could not be resolved — e.g. a descent step
    /// addressed a non-table block, or a cell / nested-block index was out of
    /// range.
    #[error("Invalid block path: {0}")]
    InvalidBlockPath(String),
}

impl From<loro::LoroError> for MutationError {
    fn from(e: loro::LoroError) -> Self {
        MutationError::Loro(e.to_string())
    }
}

// ── Shared internal helpers ───────────────────────────────────────────────────

/// Resolves section `s`'s blocks movable list, if present.
fn section_blocks_list(sections: &LoroList, s: usize) -> Option<LoroMovableList> {
    let section = sections.get(s)?.into_container().ok()?.into_map().ok()?;
    section
        .get(KEY_BLOCKS)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()
}

/// Resolves a document-global `block_index` to its section's blocks list and the
/// block's index *within that section*.
///
/// Editor block indices are global (document order across every section); each
/// section consumes `blocks.len()` of that index space, in section order. This
/// mirrors the index space the layout assigns to `PageParagraphData::block_index`,
/// so a cursor/hit-test index resolves to the correct section. Returns
/// [`MutationError::BlockIndexOutOfRange`] when `block_index` is past the last
/// block of the last section.
pub(crate) fn resolve_section_blocks(
    loro: &LoroDoc,
    block_index: usize,
) -> Result<(LoroMovableList, usize), MutationError> {
    let sections = loro.get_list(KEY_SECTIONS);
    let mut base = 0usize;
    for s in 0..sections.len() {
        let Some(blocks_list) = section_blocks_list(&sections, s) else {
            continue; // a malformed section contributes no addressable blocks
        };
        let len = blocks_list.len();
        if block_index < base + len {
            return Ok((blocks_list, block_index - base));
        }
        base += len;
    }
    Err(MutationError::BlockIndexOutOfRange(block_index))
}

/// Navigate to the `LoroText` content container for the global `block_index`.
pub(crate) fn get_loro_text_for_block(
    loro: &LoroDoc,
    block_index: usize,
) -> Result<LoroText, MutationError> {
    let (blocks_list, local) = resolve_section_blocks(loro, block_index)?;
    let block_map = blocks_list
        .get(local)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    block_map
        .get(KEY_CONTENT)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .ok_or(MutationError::TextNotFound(block_index))
}

/// Navigate to the section's `LoroMovableList` of blocks and the `LoroMap` for
/// the global `block_index`, plus the block's index *within that section*.
///
/// Returns the list and the local index so callers can insert/delete relative to
/// the correct section, and the map so they can read/write block properties.
pub(crate) fn get_block_map_and_list(
    loro: &LoroDoc,
    block_index: usize,
) -> Result<(LoroMovableList, LoroMap, usize), MutationError> {
    let (blocks_list, local) = resolve_section_blocks(loro, block_index)?;
    let block_map = blocks_list
        .get(local)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    Ok((blocks_list, block_map, local))
}

/// Copies primitive (non-container) key-value pairs from `src` to `dst`.
///
/// Container-typed values (nested LoroMaps, etc.) are silently skipped since
/// `KEY_PARA_PROPS` and `KEY_DIRECT_CHAR_PROPS` only contain primitive entries.
pub(crate) fn copy_map_primitive_values(src: &LoroMap, dst: &LoroMap) -> Result<(), MutationError> {
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
