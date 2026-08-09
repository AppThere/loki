// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reuse-boundary helpers for incremental relayout (split from
//! `incremental.rs` for the 300-line ceiling): the block-list prefix/suffix
//! diff used to locate the changed range, and the checkpoint lookup that maps
//! a section to its starting page. All are re-imported by `incremental.rs`.

use std::sync::atomic::{AtomicU64, Ordering};

use loki_doc_model::content::block::Block;

use super::PageStart;

/// Length of the longest common prefix of `old` and `new` (the index of the
/// first differing block). Works across length changes (block insert/delete).
/// Deep block-equality comparisons performed by the diff scans, process-wide.
///
/// A `Block` comparison is a structural compare of a whole paragraph, so these
/// dominate the diff's cost and are the unit worth counting — not calls.
///
/// # Why this is counted rather than reasoned about
///
/// The spread in incremental relayout cost across edit positions (24.1 / 14.4 /
/// 11.4 ms at 2500 blocks) was attributed to this scan cost, and that attribution
/// did not survive contact with the code: `common_prefix_len` scans from the front
/// and `common_suffix_len` from the back, so between them the work is roughly
/// constant whatever the edit position — they should cancel, not produce a 2x
/// spread. The domain measured was time; the conclusion was about which function
/// dominates (L9-018). This counter closes that gap instead of arguing about it.
///
/// Relaxed ordering: the value is read after the work, and exactness across
/// threads is not needed for a diagnostic that reports magnitudes.
static BLOCK_COMPARISONS: AtomicU64 = AtomicU64::new(0);

/// Comparisons counted since the last [`reset_block_comparisons`].
#[must_use]
pub fn block_comparisons() -> u64 {
    BLOCK_COMPARISONS.load(Ordering::Relaxed)
}

/// Zeroes the counter so one operation can be measured in isolation.
pub fn reset_block_comparisons() {
    BLOCK_COMPARISONS.store(0, Ordering::Relaxed);
}

#[inline]
fn note_comparisons(n: usize) {
    BLOCK_COMPARISONS.fetch_add(n as u64, Ordering::Relaxed);
}

pub(super) fn common_prefix_len(old: &[Block], new: &[Block]) -> usize {
    let max = old.len().min(new.len());
    let n = (0..max).take_while(|&i| old[i] == new[i]).count();
    // `take_while` compares one past the run unless it exhausted the range.
    note_comparisons(if n < max { n + 1 } else { n });
    n
}

/// Length of the longest common suffix of `old` and `new` that does not overlap
/// the common prefix. Used to bound the changed region for block insert/delete.
pub(super) fn common_suffix_len(old: &[Block], new: &[Block], prefix: usize) -> usize {
    let max = old.len().min(new.len()) - prefix;
    let n = (0..max)
        .take_while(|&i| old[old.len() - 1 - i] == new[new.len() - 1 - i])
        .count();
    note_comparisons(if n < max { n + 1 } else { n });
    n
}

/// Returns `true` when `old[from..]` and `new[from..]` are element-wise equal —
/// i.e. every block from `from` onward is unchanged. Used to license suffix
/// reuse: equal trailing blocks + an equal checkpoint ⇒ identical trailing pages.
pub(super) fn blocks_equal_from(old: &[Block], new: &[Block], from: usize) -> bool {
    if old.len() != new.len() {
        return true_if_never(); // length mismatch: no element compares happen
    }
    // Written as an explicit loop rather than a slice `==` so the comparisons can
    // be counted; the slice form hides how far it actually scanned, which is the
    // quantity in question.
    let mut compared = 0usize;
    let mut equal = true;
    for i in from..old.len() {
        compared += 1;
        if old[i] != new[i] {
            equal = false;
            break;
        }
    }
    note_comparisons(compared);
    equal
}

/// A length mismatch means the slices cannot be equal and no element comparison
/// is performed. Named rather than inlined so the counter's contract — it counts
/// *element* comparisons — stays legible at the early return.
#[inline]
fn true_if_never() -> bool {
    false
}

/// Global page index where `section` begins, from its block-0 checkpoint.
pub(super) fn section_page_start(checkpoints: &[PageStart], section: usize) -> Option<usize> {
    checkpoints
        .iter()
        .find(|c| c.section_index == section && c.block_index == 0)
        .map(|c| c.page_index)
}
