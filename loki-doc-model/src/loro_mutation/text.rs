// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Character-level text mutations for individual blocks.

use std::collections::HashMap;

use loro::{LoroDoc, LoroText, LoroValue, TextDelta};

use super::{MutationError, get_loro_text_for_block};
#[cfg(feature = "serde")]
use crate::content::inline::Inline;
use crate::loro_schema::CHAR_MARK_KEYS;

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

/// Replaces `len` UTF-8 bytes at `byte_offset` with `replacement`, **preserving
/// the replaced text's character formatting**.
///
/// A plain delete-then-insert loses formatting here: every mark is configured
/// with `expand: After` (see `document_to_loro`), so inserting at a position
/// that coincides with the end of a preceding run makes that run's marks
/// "swallow" the inserted text (e.g. replacing a black word right after a red
/// run turns the new word red). This captures the formatting at `byte_offset`
/// before the edit and re-applies it to the inserted range afterwards — clearing
/// any mark that leaked in and setting the ones the original text carried — so
/// the replacement matches the text it replaced and neighbours are untouched.
///
/// Intended for word-level replacements (e.g. a spelling suggestion), where the
/// replaced range has uniform formatting.
///
/// # Errors
///
/// As for [`insert_text`] / [`delete_text`], plus [`MutationError::Loro`] from
/// the mark operations.
pub fn replace_text(
    loro: &LoroDoc,
    block_index: usize,
    byte_offset: usize,
    len: usize,
    replacement: &str,
) -> Result<(), MutationError> {
    let text = get_loro_text_for_block(loro, block_index)?;
    // Capture the formatting of the range being replaced (read at its start; a
    // word has uniform formatting).
    let original_attrs = attrs_at(&text, byte_offset);
    if len > 0 {
        text.delete_utf8(byte_offset, len)?;
    }
    if replacement.is_empty() {
        return Ok(());
    }
    text.insert_utf8(byte_offset, replacement)?;
    let end = byte_offset + replacement.len();

    // Reset the inserted range to *exactly* the replaced text's formatting. We
    // cannot detect what leaked in by reading back — `expand` is applied at
    // delta/export time, so a mark that swallowed the insert is not yet visible
    // — so set every known mark key unconditionally: to the captured value, or
    // `Null` to clear it. This overrides any expansion from a neighbouring run
    // without touching the surrounding text.
    for &key in CHAR_MARK_KEYS {
        let value = original_attrs.get(key).cloned().unwrap_or(LoroValue::Null);
        text.mark_utf8(byte_offset..end, key, value)?;
    }
    Ok(())
}

/// Returns the mark attributes active on the character at `byte_offset`.
fn attrs_at(text: &LoroText, byte_offset: usize) -> HashMap<String, LoroValue> {
    let mut byte_pos = 0usize;
    for delta in text.to_delta() {
        if let TextDelta::Insert { insert, attributes } = delta {
            let span_bytes = insert.len();
            if byte_offset < byte_pos + span_bytes {
                return attributes
                    .map(|a| a.into_iter().collect())
                    .unwrap_or_default();
            }
            byte_pos += span_bytes;
        }
    }
    HashMap::new()
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

/// Inserts an inline image at UTF-8 `byte_offset` in top-level block
/// `block_index` — a thin wrapper over [`super::insert_inline_image_at`] with a
/// flat [`super::BlockPath`]. See that function for the encoding and errors.
#[cfg(feature = "serde")]
pub fn insert_inline_image(
    loro: &LoroDoc,
    block_index: usize,
    byte_offset: usize,
    image: &Inline,
) -> Result<(), MutationError> {
    super::insert_inline_image_at(
        loro,
        &super::BlockPath::block(block_index),
        byte_offset,
        image,
    )
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

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
