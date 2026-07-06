// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-entry lookup helpers for [`super::navigation`]: path-aware
//! entry search, nested-sibling resolution, and the cross-page prev/next
//! walks. Extracted to keep `navigation.rs` under the 300-line ceiling.

use loki_doc_model::PathStep;
use loki_layout::{PageParagraphData, PaginatedLayout};

use super::cursor::DocumentPosition;

/// Look up the [`PageParagraphData`] for a paragraph on a specific page.
///
/// `block_index` is the flat document block index stored in
/// [`DocumentPosition::paragraph_index`]; `path` is the nested descent
/// ([`DocumentPosition::path`]). Nested paragraphs (table cells, note bodies)
/// share their root's `block_index`, so both must match — matching on the
/// index alone would return the first cell's entry for *every* paragraph of
/// a table.
pub(super) fn find_para_data<'a>(
    layout: &'a PaginatedLayout,
    page_index: usize,
    block_index: usize,
    path: &[PathStep],
) -> Option<&'a PageParagraphData> {
    layout
        .pages
        .get(page_index)?
        .editing_data
        .as_ref()?
        .paragraphs
        .iter()
        .find(|p| p.block_index == block_index && p.path == path)
}

/// The leaf block index of a nested position within its container (the leaf
/// step's block index; positions with an empty path are not nested).
pub(super) fn nested_leaf(focus: &DocumentPosition) -> Option<usize> {
    match focus.path.last() {
        Some(PathStep::Cell { block, .. } | PathStep::Note { block, .. }) => Some(*block),
        None => None,
    }
}

/// The page index of a nested paragraph entry, searching **every** page.
///
/// A table cell's blocks can flow across a page break, so the sibling of the
/// focus need not live on the focus's page; a page-local search would miss it
/// and strand the caret. Returns the first page whose editing data holds the
/// `(block_index, path)` entry.
fn nested_para_page(
    layout: &PaginatedLayout,
    block_index: usize,
    path: &[PathStep],
) -> Option<usize> {
    layout.pages.iter().position(|page| {
        page.editing_data.as_ref().is_some_and(|ed| {
            ed.paragraphs
                .iter()
                .any(|p| p.block_index == block_index && p.path == path)
        })
    })
}

/// The sibling of a nested `focus` shifted by `delta` blocks within its
/// container, if it exists in the layout. Clamps at the container's first/last
/// block by returning `None` — crossing out of a cell or note body into the
/// surrounding document is handled by the caller's top-level fallback.
///
/// The sibling may have flowed onto a different page than the focus (a cell
/// split across a page break), so its `page_index` is re-derived from wherever
/// its entry actually appears rather than inherited from the focus.
pub(super) fn nested_sibling(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
    delta: isize,
) -> Option<DocumentPosition> {
    let leaf = nested_leaf(focus)?;
    leaf.checked_add_signed(delta)?; // clamp at the container start
    let sibling = focus.sibling_block(delta, 0);
    // Only cross when the sibling paragraph actually exists somewhere in the
    // layout, and adopt the page it was laid out on.
    let page = nested_para_page(layout, sibling.paragraph_index, &sibling.path)?;
    Some(DocumentPosition {
        page_index: page,
        ..sibling
    })
}

/// Find the paragraph entry immediately preceding `block_index` in document order.
///
/// Searches the current page first (for `block_index - 1`), then walks backward
/// through previous pages. When the preceding block is a container (a table),
/// its **last** paragraph entry on the page is returned, so entering it from
/// below lands in its final cell. Returns `(page_index, para_data)`.
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
    for pi in (0..=page_index.min(layout.pages.len().saturating_sub(1))).rev() {
        if let Some(ed) = layout.pages[pi].editing_data.as_ref()
            && let Some(para) = ed
                .paragraphs
                .iter()
                .rev()
                .find(|p| p.block_index == prev_block)
        {
            return Some((pi, para));
        }
    }
    None
}

/// Find the paragraph entry immediately following `block_index` in document order.
///
/// Searches the current page first (for `block_index + 1`), then walks forward
/// through subsequent pages. When the following block is a container (a
/// table), its **first** paragraph entry is returned, so entering it from
/// above lands in its first cell. Returns `(page_index, para_data)`.
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
