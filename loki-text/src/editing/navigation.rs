// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cursor navigation for the document editor.
//!
//! All functions work in **layout points** (the coordinate space shared by
//! `PaginatedLayout`, `cursor_rect`, and `hit_test_page`).  CSS-pixel
//! conversion happens upstream in the event handler.
//!
//! # Cross-page navigation
//!
//! All six navigation functions clamp at the current page boundary and return
//! `None` when the target falls off the page. The caller leaves the cursor
//! unchanged on `None`.  Cross-page traversal is tracked as
//! `TODO(3b-3)` throughout this file.
//!
//! # Double-Enter to exit a list
//!
//! When `split_block` is used on a list-item block the new block inherits
//! `KEY_PARA_PROPS` (including list membership). Pressing Enter twice to exit
//! the list therefore requires a subsequent `clear_para_props` call to strip
//! the list properties from the trailing empty block.
//! `TODO(3b-3)`: implement `clear_para_props` / list-exit heuristic.

use loki_doc_model::{BlockPath, PathStep};
use loki_layout::{PageParagraphData, PaginatedLayout};

use super::cursor::{DocumentPosition, next_grapheme_boundary, prev_grapheme_boundary};
use super::hit_test::hit_test_page;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Look up the [`PageParagraphData`] for a paragraph on a specific page.
///
/// `block_index` is the flat document block index stored in
/// [`DocumentPosition::paragraph_index`]; `path` is the nested descent
/// ([`DocumentPosition::path`]). Nested paragraphs (table cells, note bodies)
/// share their root's `block_index`, so both must match — matching on the
/// index alone would return the first cell's entry for *every* paragraph of
/// a table.
fn find_para_data<'a>(
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
fn nested_leaf(focus: &DocumentPosition) -> Option<usize> {
    match focus.path.last() {
        Some(PathStep::Cell { block, .. } | PathStep::Note { block, .. }) => Some(*block),
        None => None,
    }
}

/// The sibling of a nested `focus` shifted by `delta` blocks within its
/// container, if it exists in the page's layout. Clamps at the container's
/// first/last block by returning `None` — crossing out of a cell or note body
/// into the surrounding document is a separate feature.
fn nested_sibling(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
    delta: isize,
) -> Option<DocumentPosition> {
    let leaf = nested_leaf(focus)?;
    leaf.checked_add_signed(delta)?; // clamp at the container start
    let sibling = focus.sibling_block(delta, 0);
    // Only cross when the sibling paragraph actually exists in the layout.
    find_para_data(
        layout,
        sibling.page_index,
        sibling.paragraph_index,
        &sibling.path,
    )?;
    Some(sibling)
}

/// Find the paragraph entry immediately preceding `block_index` in document order.
///
/// Searches the current page first (for `block_index - 1`), then walks backward
/// through previous pages. Returns `(page_index, para_data)`.
///
/// Returns `None` when `block_index` is 0 (no predecessor exists).
fn find_prev_para_data(
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
fn find_next_para_data(
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

// ── Public navigation functions ───────────────────────────────────────────────

/// Move one grapheme cluster to the left.
///
/// - Within a paragraph: moves to the previous grapheme boundary.
/// - At offset 0: moves to the end of the previous paragraph **on the same
///   page** — the previous sibling within the same container for a nested
///   position (table cell / note body), which clamps at the container's
///   first block.
///
/// Returns `None` when already at the very start (first block on first page)
/// or when no layout data is available.
///
/// `get_text(path)` is called to retrieve the plain text of the addressed
/// paragraph. Pass a closure over [`loki_doc_model::get_block_text_at`].
///
/// `TODO(3b-3)`: when at offset 0 of the first block on a page, navigate to
/// the last character of the last block on the previous page.
pub fn navigate_left(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
    get_text: impl Fn(&BlockPath) -> String,
) -> Option<DocumentPosition> {
    if focus.byte_offset > 0 {
        let text = get_text(&focus.block_path());
        let new_offset = prev_grapheme_boundary(&text, focus.byte_offset);
        return Some(DocumentPosition {
            byte_offset: new_offset,
            ..focus.clone()
        });
    }

    // At start of a nested paragraph — move to the end of the previous
    // sibling within the same cell / note body (clamped at the first block).
    if !focus.path.is_empty() {
        let sibling = nested_sibling(focus, layout, -1)?;
        let prev_text = get_text(&sibling.block_path());
        return Some(DocumentPosition {
            byte_offset: prev_text.len(),
            ..sibling
        });
    }

    // At start of paragraph — move to end of previous block on the same page.
    if focus.paragraph_index == 0 {
        // TODO(3b-3): navigate to last char of last block on previous page
        return None;
    }
    let prev_index = focus.paragraph_index - 1;
    // Verify the previous block is actually on this page before crossing.
    find_para_data(layout, focus.page_index, prev_index, &[])?;
    let prev_text = get_text(&BlockPath::block(prev_index));
    Some(DocumentPosition::top_level(
        focus.page_index,
        prev_index,
        prev_text.len(),
    ))
}

/// Move one grapheme cluster to the right.
///
/// - Within a paragraph: moves to the next grapheme boundary.
/// - At the end: moves to the start of the next paragraph **on the same page**
///   — the next sibling within the same container for a nested position,
///   which clamps at the container's last block.
///
/// Returns `None` when already at the very end or no layout data is available.
///
/// `TODO(3b-3)`: cross-page rightward navigation.
pub fn navigate_right(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
    get_text: impl Fn(&BlockPath) -> String,
) -> Option<DocumentPosition> {
    let text = get_text(&focus.block_path());
    if focus.byte_offset < text.len() {
        let new_offset = next_grapheme_boundary(&text, focus.byte_offset);
        return Some(DocumentPosition {
            byte_offset: new_offset,
            ..focus.clone()
        });
    }

    // At the end of a nested paragraph — move to the start of the next
    // sibling within the same cell / note body (clamped at the last block).
    if !focus.path.is_empty() {
        return nested_sibling(focus, layout, 1);
    }

    // At end of paragraph — move to start of next block on the same page.
    let next_index = focus.paragraph_index + 1;
    // Verify the next block is actually on this page.
    find_para_data(layout, focus.page_index, next_index, &[])?;
    // TODO(3b-3): cross-page rightward navigation
    Some(DocumentPosition::top_level(focus.page_index, next_index, 0))
}

/// Move the cursor one line up, preserving the horizontal screen position.
///
/// Uses `cursor_rect` to find the current line's geometry, then subtracts one
/// line height and re-hits the layout.  When the target falls above the content
/// area (first line of a paragraph or page break), navigates to the last line
/// of the previous paragraph, crossing page boundaries as needed.
///
/// Returns `None` when already at the very first position of the document.
pub fn navigate_up(focus: &DocumentPosition, layout: &PaginatedLayout) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index, &focus.path)?;
    let rect = para_data.layout.cursor_rect(focus.byte_offset)?;
    let margins = &layout.pages.get(focus.page_index)?.margins;

    // Convert paragraph-local → page-canvas-local (what hit_test_page expects).
    let canvas_x = rect.x + para_data.origin.0 + margins.left;
    let canvas_y = rect.y + para_data.origin.1 + margins.top;

    // Aim for the vertical centre of the line above.
    let target_y = canvas_y - rect.height;

    // Only use hit_test_page when target_y is within the page content area.
    // If target_y < margins.top, the target is above the content area; calling
    // hit_test_page could match a split-paragraph fragment with negative origin
    // and land in its invisible region (Bug 3).
    // Avoid returning the same position (can happen when rect.height is
    // small relative to the gap between paragraphs).
    if target_y >= margins.top
        && let Some(pos) = hit_test_page(focus.page_index, canvas_x, target_y, layout)
        && (pos.paragraph_index != focus.paragraph_index || pos.byte_offset != focus.byte_offset)
    {
        return Some(pos);
    }

    // Cross-paragraph: navigate to the bottom of the previous paragraph.
    let (prev_pi, prev_para) =
        find_prev_para_data(layout, focus.page_index, focus.paragraph_index)?;
    let prev_margins = &layout.pages[prev_pi].margins;
    // Hit the last line of the previous paragraph at the same horizontal position.
    let prev_bottom_content_y = prev_para.origin.1 + prev_para.layout.height - 0.5;
    let prev_canvas_y = prev_bottom_content_y + prev_margins.top;
    hit_test_page(prev_pi, canvas_x, prev_canvas_y, layout)
}

/// Move the cursor one line down, preserving the horizontal screen position.
///
/// When the target falls below the page content area (last line of a paragraph
/// or page break), navigates to the first line of the next paragraph, crossing
/// page boundaries as needed.
///
/// Returns `None` when already at the very last position of the document.
pub fn navigate_down(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index, &focus.path)?;
    let rect = para_data.layout.cursor_rect(focus.byte_offset)?;
    let margins = &layout.pages.get(focus.page_index)?.margins;
    let page = layout.pages.get(focus.page_index)?;

    let canvas_x = rect.x + para_data.origin.0 + margins.left;
    let canvas_y = rect.y + para_data.origin.1 + margins.top;

    // Aim for the vertical centre of the line below (1.5 × line height down).
    let target_y = canvas_y + rect.height * 1.5;

    // Page bottom in canvas coords (content area bottom).
    let page_bottom = page.page_size.height - margins.bottom;

    if target_y < page_bottom
        && let Some(pos) = hit_test_page(focus.page_index, canvas_x, target_y, layout)
        && (pos.paragraph_index != focus.paragraph_index || pos.byte_offset != focus.byte_offset)
    {
        return Some(pos);
    }

    // Cross-paragraph: navigate to the top of the next paragraph.
    let (next_pi, next_para) =
        find_next_para_data(layout, focus.page_index, focus.paragraph_index)?;
    let next_margins = &layout.pages[next_pi].margins;
    // Hit slightly inside the first line of the next paragraph.
    let next_top_content_y = next_para.origin.1 + 0.5;
    let next_canvas_y = next_top_content_y + next_margins.top;
    hit_test_page(next_pi, canvas_x, next_canvas_y, layout)
}

/// Move the cursor to the start of the current visual line.
///
/// Uses `cursor_rect` to determine the line's y position, then hit-tests at
/// x = 0 on the same line.
///
/// `TODO(3b-3)`: use Parley `line_boundaries` for true Home behaviour with
/// bidirectional text.
pub fn navigate_home(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index, &focus.path)?;
    let rect = para_data.layout.cursor_rect(focus.byte_offset)?;

    // Centre-y of the current line in paragraph-local coordinates.
    let line_center_y = rect.y + rect.height / 2.0;
    let hit = para_data.layout.hit_test_point(0.0, line_center_y)?;
    Some(DocumentPosition {
        byte_offset: hit.byte_offset,
        ..focus.clone()
    })
}

/// Move the cursor to the end of the current visual line.
///
/// Uses [`ParagraphLayout::line_end_offset`] to find the last byte on the
/// line, trimming any trailing hard-break character so the cursor stays after
/// the last visible glyph.
///
/// `get_text(path)` is called to retrieve the paragraph text needed by
/// `line_end_offset` to check for a trailing `\n`.
pub fn navigate_end(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
    get_text: impl Fn(&BlockPath) -> String,
) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index, &focus.path)?;
    let text = get_text(&focus.block_path());
    let end_offset = para_data.layout.line_end_offset(focus.byte_offset, &text)?;
    Some(DocumentPosition {
        byte_offset: end_offset,
        ..focus.clone()
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
