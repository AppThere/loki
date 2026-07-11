// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table-of-contents CRDT mutations (References tab, Spec 04 M5 / plan 4a.2).
//!
//! [`insert_table_of_contents`] builds a [`TableOfContentsBlock`] from the
//! document's current headings and inserts it after a block;
//! [`refresh_table_of_contents`] rebuilds an existing TOC's snapshot in place
//! (the "update field" action). A [`Block::TableOfContents`] has no flat-text
//! schema, so it round-trips through the bridge as an opaque JSON snapshot
//! (`loro_bridge::opaque`) — refresh therefore just rewrites that snapshot.

use loro::LoroDoc;

use super::block_edit::insert_block_after;
use super::{MutationError, get_block_map_and_list};
use crate::content::block::Block;
use crate::content::toc::build_toc;
use crate::loro_bridge::loro_to_document;
use crate::loro_schema::{BLOCK_TYPE_OPAQUE, KEY_OPAQUE_JSON, KEY_TYPE};

/// Builds a table of contents from the document's headings (down to `max_depth`,
/// with an optional localised `title`) and inserts it immediately after the block
/// at `after_block_index`, returning the new block's document-global index.
///
/// # Errors
///
/// - [`MutationError::Loro`] — the document could not be rebuilt from the CRDT.
/// - [`MutationError::BlockIndexOutOfRange`] — `after_block_index` is out of range.
/// - [`MutationError::Encode`] — the TOC block could not be serialised.
pub fn insert_table_of_contents(
    loro: &LoroDoc,
    after_block_index: usize,
    title: Option<&str>,
    max_depth: u8,
) -> Result<usize, MutationError> {
    let doc = loro_to_document(loro).map_err(|e| MutationError::Loro(e.to_string()))?;
    let toc = build_toc(&doc.sections, title, max_depth);
    insert_block_after(loro, after_block_index, &Block::TableOfContents(toc))
}

/// Rebuilds the snapshot of the table of contents at `block_index` from the
/// document's current headings — the "update field" action.
///
/// A **guarded no-op** when the block at `block_index` is not a stored TOC
/// snapshot (matching the crate's other invalid-target mutations), so a stale UI
/// index can never clobber an unrelated block.
///
/// # Errors
///
/// - [`MutationError::Loro`] — the document could not be rebuilt from the CRDT.
/// - [`MutationError::BlockIndexOutOfRange`] — `block_index` is out of range.
/// - [`MutationError::Encode`] — the rebuilt TOC block could not be serialised.
pub fn refresh_table_of_contents(
    loro: &LoroDoc,
    block_index: usize,
    title: Option<&str>,
    max_depth: u8,
) -> Result<(), MutationError> {
    let (_, block_map, _) = get_block_map_and_list(loro, block_index)?;
    if !block_is_toc_snapshot(&block_map) {
        return Ok(());
    }
    let doc = loro_to_document(loro).map_err(|e| MutationError::Loro(e.to_string()))?;
    let toc = build_toc(&doc.sections, title, max_depth);
    let json = serde_json::to_string(&Block::TableOfContents(toc))
        .map_err(|e| MutationError::Encode(e.to_string()))?;
    block_map.insert(KEY_OPAQUE_JSON, json)?;
    Ok(())
}

/// Whether `block_map` is an opaque snapshot whose payload is a `TableOfContents`.
fn block_is_toc_snapshot(block_map: &loro::LoroMap) -> bool {
    let is_opaque = block_map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .is_some_and(|s| s.as_str() == BLOCK_TYPE_OPAQUE);
    if !is_opaque {
        return false;
    }
    block_map
        .get(KEY_OPAQUE_JSON)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .and_then(|s| serde_json::from_str::<Block>(&s).ok())
        .is_some_and(|b| matches!(b, Block::TableOfContents(_)))
}

/// The document-global index of the first [`Block::TableOfContents`], if any —
/// the block the editor's "update field" action refreshes.
#[must_use]
pub fn first_toc_block_index(sections: &[crate::layout::section::Section]) -> Option<usize> {
    sections
        .iter()
        .flat_map(|s| &s.blocks)
        .position(|b| matches!(b, Block::TableOfContents(_)))
}

#[cfg(test)]
#[path = "toc_tests.rs"]
mod tests;
