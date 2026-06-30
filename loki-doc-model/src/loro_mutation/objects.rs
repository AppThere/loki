// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inserts of structured inline objects (images, footnotes/endnotes) into a
//! block's live text, addressed by a [`BlockPath`] so the target may be a
//! top-level paragraph, a table cell, or a note body.
//!
//! Both objects are anchored by a single `OBJECT_REPLACEMENT_CHAR`: an image
//! carries its whole `serde`-JSON snapshot in a [`MARK_IMAGE`] mark, while a
//! note carries a `(kind, idx)` pair in a [`MARK_NOTE`] mark with its body
//! stored as a live container under the block's `KEY_NOTES` list (the bridge's
//! `loro_bridge::inline_objects` owns that schema; this module addresses where
//! the anchor goes and delegates the body write to it).

use loro::{LoroDoc, LoroValue};

use super::nested::{resolve_block_map, text_for_path};
use super::{BlockPath, MutationError, insert_text_at, mark_text_at};
use crate::content::block::Block;
use crate::content::inline::{Inline, NoteKind};
use crate::loro_schema::{MARK_IMAGE, OBJECT_REPLACEMENT_STR};

/// Inserts an inline image at `byte_offset` in the block addressed by `path`.
///
/// The image becomes an `OBJECT_REPLACEMENT_CHAR` anchor + [`MARK_IMAGE`]
/// snapshot (the bridge's native encoding) inside the addressed paragraph —
/// which may be a table cell or note body. `image` must be an `Inline::Image`.
pub fn insert_inline_image_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    image: &Inline,
) -> Result<(), MutationError> {
    if !matches!(image, Inline::Image(..)) {
        return Err(MutationError::Encode("not an Inline::Image".to_string()));
    }
    let json = serde_json::to_string(image).map_err(|e| MutationError::Encode(e.to_string()))?;
    insert_text_at(loro, path, byte_offset, OBJECT_REPLACEMENT_STR)?;
    let end = byte_offset + OBJECT_REPLACEMENT_STR.len();
    mark_text_at(
        loro,
        path,
        byte_offset,
        end,
        MARK_IMAGE,
        LoroValue::from(json),
    )?;
    Ok(())
}

/// Inserts a footnote/endnote at `byte_offset` in the block addressed by `path`,
/// with `body` as its initial content (typically one empty paragraph the user
/// then edits).
///
/// The anchor is an `OBJECT_REPLACEMENT_CHAR` marked with `(kind, idx)`; the
/// body is written as a live container under the block's `KEY_NOTES` list, so
/// it round-trips through the bridge and is reachable for nested editing via a
/// [`BlockPath`] step `PathStep::Note { note: idx, .. }`.
///
/// # Errors
///
/// [`MutationError::InvalidBlockPath`] when `path` does not resolve,
/// [`MutationError::TextNotFound`] when the target block has no text, or
/// [`MutationError::Encode`] when the bridge cannot write the note.
pub fn insert_inline_note_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    kind: &NoteKind,
    body: &[Block],
) -> Result<(), MutationError> {
    let block_map = resolve_block_map(loro, path)?;
    // `text_for_path` resolves (and validates) the same block's `LoroText`.
    let text = text_for_path(loro, path)?;
    crate::loro_bridge::insert_note_at(&text, &block_map, byte_offset, kind, body)
        .map_err(|e| MutationError::Encode(e.to_string()))
}
