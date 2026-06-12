// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Incremental `Document` reconstruction from a `LoroDoc`.
//!
//! [`super::loro_to_document`] rebuilds the entire [`Document`] on every call —
//! `O(total content)` per keystroke, which dominates the edit path once the
//! per-paragraph layout is cached. [`IncrementalReader`] keeps the last
//! reconstructed [`Document`] together with the Loro version it reflects; on
//! [`IncrementalReader::update`] it diffs the new version against the old and,
//! when every change is confined to the text / marks / properties of existing
//! blocks in section 0 (the common typing case), re-reconstructs only those
//! blocks.
//!
//! Anything else — a block insert/delete/move, a section/layout/metadata change,
//! a diff error, or a change it cannot map to a single block — falls back to a
//! full [`super::loro_to_document`]. The returned document is therefore always
//! identical to a full rebuild; only the *work* is reduced.
//!
//! ## Path mapping
//!
//! Loro's `get_path_to_container` returns the path root→target as
//! `(container_id, index_of_that_container_within_its_parent)` tuples. For the
//! text of block *N* in section 0 the path passes through
//! `(blocks_list, Key("blocks"))` immediately followed by
//! `(block_map, Seq(N))`, so the block index is the `Seq` of the element right
//! after the blocks-list element. A change to the blocks list itself (an insert
//! or delete) has no such following element and is treated as structural.

use loro::{ContainerID, ContainerTrait, Frontiers, Index, LoroDoc, LoroMovableList};

use super::BridgeError;
use super::read::map_loro_block;
use crate::document::Document;
use crate::loro_schema::{KEY_BLOCKS, KEY_SECTIONS};

/// Stateful incremental reconstructor. Owns the last-derived [`Document`] and
/// the Loro version it corresponds to.
pub struct IncrementalReader {
    cached: Document,
    version: Frontiers,
}

impl IncrementalReader {
    /// Seeds the reader with a full reconstruction at the doc's current version.
    ///
    /// Commits first so the captured version includes any pending local edits.
    pub fn seed(loro: &LoroDoc) -> Result<Self, BridgeError> {
        loro.commit();
        let cached = super::loro_to_document(loro)?;
        Ok(Self {
            cached,
            version: loro.state_frontiers(),
        })
    }

    /// Returns the current document, reconstructing only the blocks that changed
    /// since the last call when possible (otherwise a full rebuild).
    pub fn update(&mut self, loro: &LoroDoc) -> Result<&Document, BridgeError> {
        loro.commit();
        let now = loro.state_frontiers();
        if now == self.version {
            return Ok(&self.cached);
        }
        if !self.try_incremental(loro, &now) {
            self.cached = super::loro_to_document(loro)?;
        }
        self.version = now;
        Ok(&self.cached)
    }

    /// Attempts a block-local incremental update. Returns `false` to request a
    /// full rebuild (leaving `self.cached` untouched or only partially patched —
    /// the caller overwrites it entirely in that case).
    fn try_incremental(&mut self, loro: &LoroDoc, now: &Frontiers) -> bool {
        let Some(blocks_list) = section0_blocks_list(loro) else {
            return false;
        };
        let blocks_id = blocks_list.id();

        let Ok(diff) = loro.diff(&self.version, now) else {
            return false;
        };

        // Collect the distinct section-0 block indices touched by this diff.
        let mut dirty: Vec<usize> = Vec::new();
        for (cid, _diff) in diff.iter() {
            if *cid == blocks_id {
                return false; // structural change to the block list itself
            }
            match block_index_for(loro, cid, &blocks_id) {
                Some(n) => {
                    if !dirty.contains(&n) {
                        dirty.push(n);
                    }
                }
                None => return false, // change outside section-0 block contents
            }
        }
        if dirty.is_empty() {
            return false;
        }

        let Some(section) = self.cached.sections.first_mut() else {
            return false;
        };
        // Index space must match: text/mark/prop edits never change block count.
        if dirty.iter().any(|&n| n >= section.blocks.len()) {
            return false;
        }
        for n in dirty {
            let Some(block_map) = blocks_list
                .get(n)
                .and_then(|v| v.into_container().ok())
                .and_then(|c| c.into_map().ok())
            else {
                return false;
            };
            match map_loro_block(&block_map) {
                Ok(block) => section.blocks[n] = block,
                Err(_) => return false,
            }
        }
        true
    }
}

/// Resolves section 0's blocks movable list, if present.
fn section0_blocks_list(loro: &LoroDoc) -> Option<LoroMovableList> {
    let sections = loro.get_list(KEY_SECTIONS);
    let section = sections.get(0)?.into_container().ok()?.into_map().ok()?;
    section
        .get(KEY_BLOCKS)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()
}

/// Returns the section-0 block index that `cid` lives inside, or `None` when
/// `cid` is not a descendant of the blocks list (or is the list itself).
///
/// The block index is the `Seq` of the path element immediately following the
/// blocks-list element (see the module docs).
fn block_index_for(loro: &LoroDoc, cid: &ContainerID, blocks_id: &ContainerID) -> Option<usize> {
    let path = loro.get_path_to_container(cid)?;
    let mut after_blocks = false;
    for (container, index) in &path {
        if after_blocks {
            return match index {
                Index::Seq(n) => Some(*n),
                _ => None,
            };
        }
        if container == blocks_id {
            after_blocks = true;
        }
    }
    None
}

#[cfg(test)]
#[path = "incremental_tests.rs"]
mod tests;
