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
//! blocks (in any section), re-reconstructs only those blocks.
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
//! text of block *N* in section *S* the path passes through
//! `(sections_list, Key("sections"))`, `(section_map, Seq(S))`,
//! `(blocks_list, Key("blocks"))`, then `(block_map, Seq(N))`. Each section's
//! blocks-list container id is collected up front, so the element immediately
//! following a known blocks list yields the block index `N`, and the matched
//! blocks list yields the section index `S`. A change *to* a blocks list (an
//! insert or delete) or to the sections list itself is treated as structural and
//! triggers a full rebuild.

use loro::{ContainerID, ContainerTrait, Frontiers, Index, LoroDoc, LoroList, LoroMovableList};

use super::BridgeError;
use super::read::map_loro_block;
use crate::document::Document;
use crate::loro_schema::{KEY_BLOCKS, KEY_SECTIONS};

/// Stateful incremental reconstructor. Owns the last-derived [`Document`] and
/// the Loro version it corresponds to.
pub struct IncrementalReader {
    cached: Document,
    version: Frontiers,
    last_was_incremental: bool,
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
            last_was_incremental: false,
        })
    }

    /// Returns the current document, reconstructing only the blocks that changed
    /// since the last call when possible (otherwise a full rebuild).
    pub fn update(&mut self, loro: &LoroDoc) -> Result<&Document, BridgeError> {
        loro.commit();
        let now = loro.state_frontiers();
        if now == self.version {
            // No change since the last call — neither a patch nor a full rebuild.
            self.last_was_incremental = true;
            return Ok(&self.cached);
        }
        if self.try_incremental(loro, &now) {
            self.last_was_incremental = true;
        } else {
            self.cached = super::loro_to_document(loro)?;
            self.last_was_incremental = false;
        }
        self.version = now;
        Ok(&self.cached)
    }

    /// Whether the most recent [`update`](Self::update) used the block-local fast
    /// path (`true`) rather than a full rebuild (`false`).
    ///
    /// Primarily for instrumentation and tests; a no-op update (no version
    /// change) reports `true` because it does no full rebuild.
    pub fn last_update_was_incremental(&self) -> bool {
        self.last_was_incremental
    }

    /// Attempts a block-local incremental update. Returns `false` to request a
    /// full rebuild (leaving `self.cached` untouched or only partially patched —
    /// the caller overwrites it entirely in that case).
    fn try_incremental(&mut self, loro: &LoroDoc, now: &Frontiers) -> bool {
        let sections = loro.get_list(KEY_SECTIONS);
        let sections_id = sections.id();

        // The blocks-list container of every section, in section order. Their
        // ids let us map a changed container back to `(section, block)`.
        let mut blocks_lists: Vec<LoroMovableList> = Vec::with_capacity(sections.len());
        for s in 0..sections.len() {
            let Some(list) = section_blocks_list(&sections, s) else {
                return false;
            };
            blocks_lists.push(list);
        }
        if blocks_lists.is_empty() {
            return false;
        }

        let Ok(diff) = loro.diff(&self.version, now) else {
            return false;
        };

        // Collect the distinct `(section, block)` pairs touched by this diff.
        let mut dirty: Vec<(usize, usize)> = Vec::new();
        for (cid, _diff) in diff.iter() {
            if *cid == sections_id {
                return false; // structural change to the sections list itself
            }
            if blocks_lists.iter().any(|l| l.id() == *cid) {
                return false; // structural change to a section's block list
            }
            match locate_block(loro, cid, &blocks_lists) {
                Some(sb) => {
                    if !dirty.contains(&sb) {
                        dirty.push(sb);
                    }
                }
                None => return false, // change outside any section's block contents
            }
        }
        if dirty.is_empty() {
            return false;
        }

        // Index space must match: text/mark/prop edits never change block count.
        for &(s, n) in &dirty {
            match self.cached.sections.get(s) {
                Some(section) if n < section.blocks.len() => {}
                _ => return false,
            }
        }

        for (s, n) in dirty {
            let Some(block_map) = blocks_lists[s]
                .get(n)
                .and_then(|v| v.into_container().ok())
                .and_then(|c| c.into_map().ok())
            else {
                return false;
            };
            let Ok(block) = map_loro_block(&block_map) else {
                return false;
            };
            let Some(slot) = self
                .cached
                .sections
                .get_mut(s)
                .and_then(|section| section.blocks.get_mut(n))
            else {
                return false;
            };
            *slot = block;
        }
        true
    }
}

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

/// Returns the `(section_index, block_index)` that `cid` lives inside, or `None`
/// when `cid` is not a descendant of any section's blocks list.
///
/// The block index is the `Seq` of the path element immediately following the
/// blocks-list element; the section index is the position of that blocks list in
/// `blocks_lists` (see the module docs).
fn locate_block(
    loro: &LoroDoc,
    cid: &ContainerID,
    blocks_lists: &[LoroMovableList],
) -> Option<(usize, usize)> {
    let path = loro.get_path_to_container(cid)?;
    let mut pending_section: Option<usize> = None;
    for (container, index) in &path {
        if let Some(section) = pending_section {
            return match index {
                Index::Seq(n) => Some((section, *n)),
                _ => None,
            };
        }
        if let Some(section) = blocks_lists.iter().position(|l| l.id() == *container) {
            pending_section = Some(section);
        }
    }
    None
}

#[cfg(test)]
#[path = "incremental_tests.rs"]
mod tests;
