// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! CRDT mutation layer for the Loki document editor.
//!
//! Provides typed helpers for inserting and deleting text within a [`LoroDoc`],
//! using the container path established by [`crate::loro_bridge::document_to_loro`].
//!
//! # Container Path
//!
//! Text for block N in section 0 lives at:
//! ```text
//! sections[0].blocks[N]["content"]  (LoroText)
//! ```
//!
//! In Session 3a all mutations target section 0.  Multi-section support
//! (mapping global block indices across section boundaries) is deferred
//! to a later session.
//!
//! # Byte Offsets
//!
//! All `byte_offset` and `len` parameters are **UTF-8 byte positions**.
//! Loro's `insert_utf8` / `delete_utf8` methods accept byte positions,
//! which matches the byte offsets produced by Parley's hit-tester and
//! stored in [`crate::loro_bridge::DocumentPosition`].

use loro::{LoroDoc, LoroText};

use crate::loro_schema::{KEY_BLOCKS, KEY_CONTENT, KEY_SECTIONS};

/// Errors that can occur when mutating a [`LoroDoc`] block's text.
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
}

impl From<loro::LoroError> for MutationError {
    fn from(e: loro::LoroError) -> Self {
        MutationError::Loro(e.to_string())
    }
}

// ── Internal helper ───────────────────────────────────────────────────────────

/// Navigate to the `LoroText` container for `block_index` in section 0.
///
/// Path: `sections_list[0] → LoroMap → blocks[block_index] → LoroMap → "content"`
///
/// Returns `Err(MutationError::BlockIndexOutOfRange)` when `block_index`
/// exceeds the number of blocks in section 0, or
/// `Err(MutationError::TextNotFound)` when the container is absent or is not
/// a `LoroText` (e.g. a table block that was stored with a stub type).
fn get_loro_text_for_block(
    loro: &LoroDoc,
    block_index: usize,
) -> Result<LoroText, MutationError> {
    // sections_list[0]
    let sections_list = loro.get_list(KEY_SECTIONS);
    let sec_val = sections_list
        .get(0)
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;
    let sec_map = sec_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())
        .ok_or(MutationError::BlockIndexOutOfRange(block_index))?;

    // blocks[block_index]
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

    // block["content"]
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

// ── Public API ────────────────────────────────────────────────────────────────

/// Inserts `text` at UTF-8 `byte_offset` into the `LoroText` for the block
/// at `block_index` (in section 0).
///
/// After a successful insertion the caller should call
/// [`apply_mutation_and_relayout`](crate::components::document_source::apply_mutation_and_relayout)
/// to re-derive the document snapshot and trigger a GPU re-render.
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
/// Used by the keyboard handler to compute grapheme boundaries without
/// running a full `loro_to_document` derivation.
///
/// Returns an empty string when `block_index` is out of range or the block
/// has no `LoroText` (rather than panicking), so callers can treat this as a
/// safe read-only probe.
pub fn get_block_text(loro: &LoroDoc, block_index: usize) -> String {
    get_loro_text_for_block(loro, block_index)
        .map(|t| t.to_string())
        .unwrap_or_default()
}
