// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Write helpers for structured inline objects anchored in a paragraph's text.
//!
//! Each object is anchored by a single [`OBJECT_REPLACEMENT_CHAR`] in the
//! block's `LoroText`, carrying a mark that identifies it:
//!
//! - **Image** ([`MARK_IMAGE`]): the whole `Inline::Image` as a `serde`-JSON
//!   snapshot in the mark (its bytes/geometry are small and immutable).
//! - **Note** ([`MARK_NOTE`]): a `(kind, idx)` pair in the mark, with the body
//!   stored as a **live CRDT container** under the block's [`KEY_NOTES`] list at
//!   `idx` — so footnote text is editable/mergeable, not a JSON blob.
//!
//! Split out of `inlines.rs` to keep that file under the 300-line ceiling.
//! Reconstructed by `inlines_read::reconstruct_inlines`.

use super::BridgeError;
use crate::content::block::Block;
use crate::content::inline::{Inline, NoteKind};
use crate::loro_schema::{KEY_NOTES, MARK_NOTE, OBJECT_REPLACEMENT_STR};
use loro::{LoroMap, LoroMovableList, LoroText};

/// Writes a self-contained inline object (currently [`Inline::Image`]) as an
/// anchor whose `serde`-JSON snapshot is stored in `mark_key`
/// (e.g. [`MARK_IMAGE`][crate::loro_schema::MARK_IMAGE]).
pub(super) fn write_inline_object(
    inline: &Inline,
    text: &LoroText,
    mark_key: &str,
) -> Result<(), BridgeError> {
    match serde_json::to_string(inline) {
        Ok(json) => {
            let start = text.len_unicode();
            text.insert(start, OBJECT_REPLACEMENT_STR)?;
            let end = text.len_unicode();
            text.mark(start..end, mark_key, json)?;
        }
        Err(err) => {
            // Unreachable in practice: `Inline` derives Serialize. Drop the
            // object rather than leave a bare anchor with no backing data.
            tracing::warn!("loro bridge: failed to encode inline object ({mark_key}): {err}");
        }
    }
    Ok(())
}

/// Writes a footnote/endnote: an [`OBJECT_REPLACEMENT_CHAR`] anchor marked with
/// a `(kind, idx)` pair, plus the **body** as a live container at `idx` in the
/// block's [`KEY_NOTES`] list. The `idx` also keeps adjacent notes' marks
/// distinct so their anchors do not merge into one rich-text span.
pub(super) fn write_note(
    kind: &NoteKind,
    body: &[Block],
    text: &LoroText,
    block_map: &LoroMap,
) -> Result<(), BridgeError> {
    let notes = get_or_create_notes_list(block_map)?;
    let idx = notes.len();

    let start = text.len_unicode();
    text.insert(start, OBJECT_REPLACEMENT_STR)?;
    let end = text.len_unicode();
    let meta =
        serde_json::to_string(&(kind, idx)).unwrap_or_else(|_| String::from("[\"Footnote\",0]"));
    text.mark(start..end, MARK_NOTE, meta)?;

    let body_list = notes.insert_container(idx, LoroMovableList::new())?;
    super::write::map_blocks_to_list(body, &body_list)?;
    Ok(())
}

/// Returns the block's [`KEY_NOTES`] movable list, creating it if absent.
fn get_or_create_notes_list(block_map: &LoroMap) -> Result<LoroMovableList, BridgeError> {
    if let Some(list) = block_map
        .get(KEY_NOTES)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
    {
        Ok(list)
    } else {
        Ok(block_map.insert_container(KEY_NOTES, LoroMovableList::new())?)
    }
}
