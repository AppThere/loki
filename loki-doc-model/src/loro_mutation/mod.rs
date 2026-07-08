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

mod align;
mod block;
mod block_edit;
mod nested;
#[cfg(feature = "serde")]
mod objects;
mod page;
mod page_style;
mod para_mark;
mod revision;
mod selection;
mod style;
#[cfg(feature = "serde")]
mod table_ops;
mod text;
mod text_containers;
#[cfg(feature = "serde")]
mod toc;

pub use self::align::{
    get_block_alignment, get_block_alignment_at, set_block_alignment, set_block_alignment_at,
};
pub use self::block::{merge_block, merge_block_at, split_block, split_block_at};
pub use self::block_edit::delete_block;
#[cfg(feature = "serde")]
pub use self::block_edit::insert_block_after;
pub use self::nested::{
    BlockPath, PathStep, delete_text_at, get_block_text_at, get_mark_at_path, insert_text_at,
    insert_text_tracked_at, mark_text_at,
};
#[cfg(feature = "serde")]
pub use self::objects::{insert_inline_image_at, insert_inline_note_at};
pub use self::page::{
    document_column_count, document_is_landscape, document_margins, document_page_size,
    set_document_columns, set_document_margins, set_document_orientation, set_document_page_size,
};
pub use self::page_style::{rename_page_style, set_page_style_geometry};
pub use self::para_mark::set_para_mark_deletion;
pub use self::revision::{
    accept_reject_all_revisions, accept_reject_revision_at, revision_at, tracked_grapheme_delete,
};
pub use self::selection::{delete_selection_at, tracked_delete_selection_at};
pub use self::style::{
    clear_block_list, get_block_list_id, get_block_style_name, set_block_style,
    set_block_type_heading, set_block_type_para,
};
#[cfg(feature = "serde")]
pub use self::table_ops::{
    delete_table_column, delete_table_row, insert_table_column, insert_table_row, table_grid_dims,
};
#[cfg(feature = "serde")]
pub use self::text::insert_inline_image;
pub use self::text::{
    delete_text, get_block_text, get_mark_at, insert_text, mark_text, replace_text,
};
#[cfg(feature = "serde")]
pub use self::toc::{first_toc_block_index, insert_table_of_contents, refresh_table_of_contents};

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
    /// A selection operation was given endpoints in different containers
    /// (body ↔ table cell, cell ↔ cell, note ↔ body, or different sections).
    /// Nothing is mutated.
    #[error("Selection endpoints are in different containers")]
    CrossContainerSelection,
    /// A structural table mutation was attempted on a table shape it does not
    /// support — merged (spanning) cells, head/foot rows, more than one body, a
    /// ragged grid, or an index out of range. Nothing is mutated.
    #[error("Unsupported table structure: {0}")]
    UnsupportedTableStructure(String),
}

impl From<loro::LoroError> for MutationError {
    fn from(e: loro::LoroError) -> Self {
        MutationError::Loro(e.to_string())
    }
}

// ── Shared internal helpers ───────────────────────────────────────────────────

/// Resolves section `s`'s blocks movable list, if present.
pub(super) fn section_blocks_list(sections: &LoroList, s: usize) -> Option<LoroMovableList> {
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
