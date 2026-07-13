// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reuse-boundary helpers for incremental relayout (split from
//! `incremental.rs` for the 300-line ceiling): the block-list prefix/suffix
//! diff used to locate the changed range, and the checkpoint lookup that maps
//! a section to its starting page. All are re-imported by `incremental.rs`.

use loki_doc_model::content::block::Block;

use super::PageStart;

/// Length of the longest common prefix of `old` and `new` (the index of the
/// first differing block). Works across length changes (block insert/delete).
pub(super) fn common_prefix_len(old: &[Block], new: &[Block]) -> usize {
    let max = old.len().min(new.len());
    (0..max).take_while(|&i| old[i] == new[i]).count()
}

/// Length of the longest common suffix of `old` and `new` that does not overlap
/// the common prefix. Used to bound the changed region for block insert/delete.
pub(super) fn common_suffix_len(old: &[Block], new: &[Block], prefix: usize) -> usize {
    let max = old.len().min(new.len()) - prefix;
    (0..max)
        .take_while(|&i| old[old.len() - 1 - i] == new[new.len() - 1 - i])
        .count()
}

/// Returns `true` when `old[from..]` and `new[from..]` are element-wise equal —
/// i.e. every block from `from` onward is unchanged. Used to license suffix
/// reuse: equal trailing blocks + an equal checkpoint ⇒ identical trailing pages.
pub(super) fn blocks_equal_from(old: &[Block], new: &[Block], from: usize) -> bool {
    old.len() == new.len() && old[from..] == new[from..]
}

/// Global page index where `section` begins, from its block-0 checkpoint.
pub(super) fn section_page_start(checkpoints: &[PageStart], section: usize) -> Option<usize> {
    checkpoints
        .iter()
        .find(|c| c.section_index == section && c.block_index == 0)
        .map(|c| c.page_index)
}
