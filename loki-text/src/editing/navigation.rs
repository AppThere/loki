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
//! Left/Right cross page boundaries (the prev/next-entry searches walk the
//! whole layout); Up/Down cross via their previous/next-paragraph fallback.
//! `None` means there is nowhere to go (document start/end) and the caller
//! leaves the cursor unchanged.
//!
//! # Double-Enter to exit a list
//!
//! When `split_block` is used on a list-item block the new block inherits
//! `KEY_PARA_PROPS` (including list membership). Pressing Enter twice to exit
//! the list therefore requires a subsequent `clear_para_props` call to strip
//! the list properties from the trailing empty block.
//! `TODO(3b-3)`: implement `clear_para_props` / list-exit heuristic.

use loki_doc_model::BlockPath;
use loki_layout::PaginatedLayout;

use super::cursor::{DocumentPosition, next_grapheme_boundary, prev_grapheme_boundary};
use super::hit_test::hit_test_page;
use super::navigation_find::{
    find_next_para_data, find_para_data, find_prev_para_data, nested_sibling,
};

// ── Public navigation functions ───────────────────────────────────────────────

/// Move one grapheme cluster to the left.
///
/// - Within a paragraph: moves to the previous grapheme boundary.
/// - At offset 0: moves to the end of the previous paragraph, crossing page
///   boundaries — or, for a nested position (table cell / note body), to the
///   previous sibling within the same container, clamping at the container's
///   first block.
///
/// Returns `None` when already at the very start (first block on first page)
/// or when no layout data is available.
///
/// `get_text(path)` is called to retrieve the plain text of the addressed
/// paragraph. Pass a closure over [`loki_doc_model::get_block_text_at`].
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

    // At start of a nested paragraph — move to the end of the previous sibling
    // within the same cell / note body. When there is no previous sibling (the
    // container's first block), fall through to the top-level prev-block search:
    // the container is addressed by its root `paragraph_index`, so that search
    // finds the block preceding it and the caret escapes the cell / note body.
    if !focus.path.is_empty()
        && let Some(sibling) = nested_sibling(focus, layout, -1)
    {
        let prev_text = get_text(&sibling.block_path());
        return Some(DocumentPosition {
            byte_offset: prev_text.len(),
            ..sibling
        });
    }

    // At start of paragraph — move to the end of the previous block,
    // crossing page boundaries (the entry search walks backward through the
    // layout, so the nearest fragment page wins for split paragraphs).
    let (prev_pi, prev_para) =
        find_prev_para_data(layout, focus.page_index, focus.paragraph_index)?;
    let prev_pos = DocumentPosition {
        page_index: prev_pi,
        paragraph_index: prev_para.block_index,
        byte_offset: 0,
        path: prev_para.path.clone(),
    };
    let prev_text = get_text(&prev_pos.block_path());
    Some(DocumentPosition {
        byte_offset: prev_text.len(),
        ..prev_pos
    })
}

/// Move one grapheme cluster to the right.
///
/// - Within a paragraph: moves to the next grapheme boundary.
/// - At the end: moves to the start of the next paragraph, crossing page
///   boundaries — or, for a nested position, to the next sibling within the
///   same container, clamping at the container's last block.
///
/// Returns `None` when already at the very end or no layout data is available.
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

    // At the end of a nested paragraph — move to the start of the next sibling
    // within the same cell / note body. When there is no next sibling (the
    // container's last block), fall through to the top-level next-block search:
    // the container is addressed by its root `paragraph_index`, so that search
    // finds the block following it and the caret escapes the cell / note body.
    if !focus.path.is_empty()
        && let Some(sibling) = nested_sibling(focus, layout, 1)
    {
        return Some(sibling);
    }

    // At end of paragraph — move to the start of the next block, crossing
    // page boundaries (the entry search walks forward through the layout).
    let (next_pi, next_para) =
        find_next_para_data(layout, focus.page_index, focus.paragraph_index)?;
    Some(DocumentPosition {
        page_index: next_pi,
        paragraph_index: next_para.block_index,
        byte_offset: 0,
        path: next_para.path.clone(),
    })
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

    // Convert paragraph-local → page-canvas-local (what hit_test_page expects),
    // mapping through the cell rotation when the caret is in a rotated cell so
    // the move aims at the caret's *visual* position (4b.5 tail).
    let (vis_x, vis_y) = para_data.local_to_page(rect.x, rect.y);
    let canvas_x = vis_x + margins.left;
    let canvas_y = vis_y + margins.top;

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
    // Hit the last line of the previous paragraph at the same horizontal
    // position (visual span, so a rotated cell's box is aimed at correctly).
    let prev_bottom_content_y = prev_para.visual_y_span().1 - 0.5;
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

    // As in `navigate_up`: rotation-aware visual caret position (4b.5 tail).
    let (vis_x, vis_y) = para_data.local_to_page(rect.x, rect.y);
    let canvas_x = vis_x + margins.left;
    let canvas_y = vis_y + margins.top;

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
    // Hit slightly inside the first line of the next paragraph (visual span,
    // so a rotated cell's box is aimed at correctly).
    let next_top_content_y = next_para.visual_y_span().0 + 0.5;
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
