// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The **flat block index**: addressing a block by one document-wide number
//! rather than by (section, block).
//!
//! Split from `document.rs` at the 300-line ceiling, on a real seam — the
//! editor and the `loro_mutation` layer both address blocks this way, and the
//! mapping between that flat space and the sectioned one is a self-contained
//! question about indices rather than about what a document is.

use super::document::Document;

impl Document {
    /// Returns an iterator over all blocks across all sections in document
    /// order.
    ///
    /// Blocks are yielded section by section, then in block order within each
    /// section. The position of a block in this iterator corresponds to its
    /// flat index as used by the Loro bridge (`block_0`, `block_1`, …).
    pub fn blocks_flat(&self) -> impl Iterator<Item = &crate::content::block::Block> {
        self.sections.iter().flat_map(|s| s.blocks.iter())
    }

    /// Returns the block at flat index `index` across all sections, or `None`
    /// if `index` is out of range.
    ///
    /// Flat indices are assigned by iterating sections in order, then blocks
    /// within each section. For example, in a document with two sections of
    /// two blocks each, flat index `2` is the first block of the second
    /// section.
    ///
    /// Flat indices are stable within a document snapshot but are **not**
    /// preserved across mutations that insert or remove blocks.
    #[must_use]
    pub fn block_at_flat(&self, index: usize) -> Option<&crate::content::block::Block> {
        self.blocks_flat().nth(index)
    }

    /// Returns the total number of blocks across all sections.
    ///
    /// Returns `0` for an empty document (no sections or all sections empty).
    #[must_use]
    pub fn block_count_flat(&self) -> usize {
        self.sections.iter().map(|s| s.blocks.len()).sum()
    }

    /// Returns the `(section_index, block_index_within_section)` pair for a
    /// given flat block index, or `None` if `flat_index` is out of range.
    ///
    /// Useful for locating which section owns a block when only its flat index
    /// is known (e.g. after receiving a Loro mutation targeting `block_N`).
    ///
    /// # Examples
    ///
    /// For a document with two sections of two blocks each:
    /// - `flat_index_to_section_block(0)` → `Some((0, 0))`
    /// - `flat_index_to_section_block(2)` → `Some((1, 0))`
    /// - `flat_index_to_section_block(4)` → `None`
    #[must_use]
    pub fn flat_index_to_section_block(&self, flat_index: usize) -> Option<(usize, usize)> {
        let mut remaining = flat_index;
        for (s_idx, section) in self.sections.iter().enumerate() {
            if remaining < section.blocks.len() {
                return Some((s_idx, remaining));
            }
            remaining -= section.blocks.len();
        }
        None
    }
}
