// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Addressing for content nested inside container blocks (table cells and
//! footnote/endnote bodies), and text mutations against it.
//!
//! The flat mutation API addresses a block by a document-global index into the
//! section block lists. That cannot reach a paragraph *inside a table cell* or
//! *inside a note body*, whose `LoroText` lives under the owning block's
//! [`KEY_TABLE_CELLS`] / [`KEY_NOTES`] container (see `loro_bridge::table` and
//! `loro_bridge::inline_objects`). A [`BlockPath`] names such a target: a root
//! block plus zero or more [`PathStep`] descents — each selecting a cell or note
//! (in the bridge's flat order) and a block within it. The path is recursive, so
//! a table nested in a cell, or a note inside a cell, is reachable too.
//!
//! Mutating the live nested `LoroText` round-trips: the bridge rebuilds each
//! cell / note body from these same containers on read.

use loro::{LoroDoc, LoroMap, LoroText, LoroValue};

use super::{MutationError, get_block_map_and_list};
use crate::loro_schema::{KEY_CONTENT, KEY_NOTES, KEY_TABLE_CELLS};

/// One descent into a container block: select a cell (of a table) or a note body
/// (of a paragraph), then a block within that nested block list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathStep {
    /// Into a table's `cell`-th cell (bridge flat head → bodies → foot order),
    /// then the `block`-th block of that cell.
    Cell {
        /// Flat cell index within the table's [`KEY_TABLE_CELLS`] list.
        cell: usize,
        /// Block index within that cell's content.
        block: usize,
    },
    /// Into a block's `note`-th footnote/endnote body, then the `block`-th block
    /// of that body.
    Note {
        /// Note index within the block's [`KEY_NOTES`] list.
        note: usize,
        /// Block index within that note's body.
        block: usize,
    },
}

impl PathStep {
    /// The container key, nested-list index, block index, and a label for errors.
    fn parts(self) -> (&'static str, usize, usize, &'static str) {
        match self {
            PathStep::Cell { cell, block } => (KEY_TABLE_CELLS, cell, block, "cell"),
            PathStep::Note { note, block } => (KEY_NOTES, note, block, "note"),
        }
    }
}

/// A path to a block, either top-level or nested inside table cell(s) / note(s).
///
/// `root` is a document-global block index (the same space the flat API and the
/// cursor use); `steps` descends through containers. An empty `steps` resolves
/// exactly like the flat API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockPath {
    /// Document-global index of the root block.
    pub root: usize,
    /// Successive container descents from the root.
    pub steps: Vec<PathStep>,
}

impl BlockPath {
    /// A top-level block (no nesting) — equivalent to the flat API.
    #[must_use]
    pub fn block(root: usize) -> Self {
        Self {
            root,
            steps: Vec::new(),
        }
    }

    /// A block at `block` inside the `cell`-th cell of the table at `root`.
    #[must_use]
    pub fn in_cell(root: usize, cell: usize, block: usize) -> Self {
        Self {
            root,
            steps: vec![PathStep::Cell { cell, block }],
        }
    }

    /// A block at `block` inside the `note`-th footnote/endnote body of `root`.
    #[must_use]
    pub fn in_note(root: usize, note: usize, block: usize) -> Self {
        Self {
            root,
            steps: vec![PathStep::Note { note, block }],
        }
    }
}

/// Descends one [`PathStep`] from a container block's map to a nested block's map.
fn descend(parent_map: &LoroMap, step: PathStep) -> Result<LoroMap, MutationError> {
    let (key, container_idx, block_idx, label) = step.parts();
    let container = parent_map
        .get(key)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
        .ok_or_else(|| MutationError::InvalidBlockPath(format!("block has no {label}s")))?;
    let inner = container
        .get(container_idx)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
        .ok_or_else(|| MutationError::InvalidBlockPath(format!("no {label} {container_idx}")))?;
    inner
        .get(block_idx)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .ok_or_else(|| {
            MutationError::InvalidBlockPath(format!(
                "no block {block_idx} in {label} {container_idx}"
            ))
        })
}

/// Resolves `path` to the block's `LoroMap`.
fn resolve_block_map(loro: &LoroDoc, path: &BlockPath) -> Result<LoroMap, MutationError> {
    let (_, mut block_map, _) = get_block_map_and_list(loro, path.root)?;
    for step in &path.steps {
        block_map = descend(&block_map, *step)?;
    }
    Ok(block_map)
}

/// Resolves `path` to the `LoroText` content container of the addressed block.
fn text_for_path(loro: &LoroDoc, path: &BlockPath) -> Result<LoroText, MutationError> {
    resolve_block_map(loro, path)?
        .get(KEY_CONTENT)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .ok_or(MutationError::TextNotFound(path.root))
}

/// Inserts `text` at UTF-8 `byte_offset` into the block addressed by `path`.
///
/// # Errors
///
/// [`MutationError::InvalidBlockPath`] when a descent step is invalid,
/// [`MutationError::TextNotFound`] when the target has no text, or
/// [`MutationError::Loro`] for an internal Loro error.
pub fn insert_text_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    text: &str,
) -> Result<(), MutationError> {
    text_for_path(loro, path)?.insert_utf8(byte_offset, text)?;
    Ok(())
}

/// Deletes `len` UTF-8 bytes at `byte_offset` from the block addressed by
/// `path`. A `len` of `0` is a no-op.
pub fn delete_text_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    len: usize,
) -> Result<(), MutationError> {
    if len == 0 {
        return Ok(());
    }
    text_for_path(loro, path)?.delete_utf8(byte_offset, len)?;
    Ok(())
}

/// Applies a mark over a UTF-8 byte range in the block addressed by `path`.
/// A `byte_start >= byte_end` range is a no-op.
pub fn mark_text_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_start: usize,
    byte_end: usize,
    mark_key: &str,
    mark_value: LoroValue,
) -> Result<(), MutationError> {
    if byte_start >= byte_end {
        return Ok(());
    }
    text_for_path(loro, path)?
        .mark_utf8(byte_start..byte_end, mark_key, mark_value)
        .map_err(MutationError::from)
}

/// Returns the plain text of the block addressed by `path` (empty when the path
/// does not resolve to a text block).
#[must_use]
pub fn get_block_text_at(loro: &LoroDoc, path: &BlockPath) -> String {
    text_for_path(loro, path)
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// Returns the value of `mark_key` at UTF-8 `byte_offset` in the block
/// addressed by `path`, or `None` if unset there.
pub fn get_mark_at_path(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    mark_key: &str,
) -> Result<Option<LoroValue>, MutationError> {
    let text = text_for_path(loro, path)?;
    let mut byte_pos = 0usize;
    for delta in text.to_delta() {
        if let loro::TextDelta::Insert { insert, attributes } = delta {
            let span_bytes = insert.len();
            if byte_offset < byte_pos + span_bytes {
                return Ok(attributes.and_then(|attrs| attrs.get(mark_key).cloned()));
            }
            byte_pos += span_bytes;
        }
    }
    Ok(None)
}
