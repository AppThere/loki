// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Internal lookup helpers for locating [`PageParagraphData`] entries by block
//! index, used by the public navigation functions.

use loki_layout::{PageParagraphData, PaginatedLayout};

/// Look up the [`PageParagraphData`] for a block on a specific page.
///
/// `block_index` is the flat document block index stored in
/// [`DocumentPosition::paragraph_index`].
pub(super) fn find_para_data(
    layout: &PaginatedLayout,
    page_index: usize,
    block_index: usize,
) -> Option<&PageParagraphData> {
    layout
        .pages
        .get(page_index)?
        .editing_data
        .as_ref()?
        .paragraphs
        .iter()
        .find(|p| p.block_index == block_index)
}

/// Find the paragraph entry immediately preceding `block_index` in document order.
///
/// Searches the current page first (for `block_index - 1`), then walks backward
/// through previous pages. Returns `(page_index, para_data)`.
///
/// Returns `None` when `block_index` is 0 (no predecessor exists).
pub(super) fn find_prev_para_data(
    layout: &PaginatedLayout,
    page_index: usize,
    block_index: usize,
) -> Option<(usize, &PageParagraphData)> {
    if block_index == 0 {
        return None;
    }
    let prev_block = block_index - 1;
    // Search this page and previous pages for the entry with prev_block.
    for pi in (0..=page_index).rev() {
        if let Some(ed) = layout.pages[pi].editing_data.as_ref()
            && let Some(para) = ed.paragraphs.iter().find(|p| p.block_index == prev_block)
        {
            return Some((pi, para));
        }
    }
    None
}

/// Find the paragraph entry immediately following `block_index` in document order.
///
/// Searches the current page first (for `block_index + 1`), then walks forward
/// through subsequent pages. Returns `(page_index, para_data)`.
///
/// Returns `None` when no following block exists in the layout.
pub(super) fn find_next_para_data(
    layout: &PaginatedLayout,
    page_index: usize,
    block_index: usize,
) -> Option<(usize, &PageParagraphData)> {
    let next_block = block_index + 1;
    for pi in page_index..layout.pages.len() {
        if let Some(ed) = layout.pages[pi].editing_data.as_ref()
            && let Some(para) = ed.paragraphs.iter().find(|p| p.block_index == next_block)
        {
            return Some((pi, para));
        }
    }
    None
}
