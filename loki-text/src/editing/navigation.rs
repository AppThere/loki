// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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

use loki_layout::{PageParagraphData, PaginatedLayout};

use super::cursor::{next_grapheme_boundary, prev_grapheme_boundary, DocumentPosition};
use super::hit_test::hit_test_page;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Look up the [`PageParagraphData`] for a block on a specific page.
///
/// `block_index` is the flat document block index stored in
/// [`DocumentPosition::paragraph_index`].
fn find_para_data(
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

// ── Public navigation functions ───────────────────────────────────────────────

/// Move one grapheme cluster to the left.
///
/// - Within a paragraph: moves to the previous grapheme boundary.
/// - At offset 0: moves to the end of the previous paragraph **on the same
///   page** if one exists.
///
/// Returns `None` when already at the very start (first block on first page)
/// or when no layout data is available.
///
/// `get_text(block_index)` is called to retrieve the plain text of a block.
/// Pass a closure over `loki_doc_model::get_block_text`.
///
/// `TODO(3b-3)`: when at offset 0 of the first block on a page, navigate to
/// the last character of the last block on the previous page.
pub fn navigate_left(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
    get_text: impl Fn(usize) -> String,
) -> Option<DocumentPosition> {
    if focus.byte_offset > 0 {
        let text = get_text(focus.paragraph_index);
        let new_offset = prev_grapheme_boundary(&text, focus.byte_offset);
        return Some(DocumentPosition { byte_offset: new_offset, ..focus.clone() });
    }

    // At start of paragraph — move to end of previous block on the same page.
    if focus.paragraph_index == 0 {
        // TODO(3b-3): navigate to last char of last block on previous page
        return None;
    }
    let prev_index = focus.paragraph_index - 1;
    // Verify the previous block is actually on this page before crossing.
    find_para_data(layout, focus.page_index, prev_index)?;
    let prev_text = get_text(prev_index);
    Some(DocumentPosition {
        page_index: focus.page_index,
        paragraph_index: prev_index,
        byte_offset: prev_text.len(),
    })
}

/// Move one grapheme cluster to the right.
///
/// - Within a paragraph: moves to the next grapheme boundary.
/// - At the end: moves to the start of the next paragraph **on the same page**
///   if one exists.
///
/// Returns `None` when already at the very end or no layout data is available.
///
/// `TODO(3b-3)`: cross-page rightward navigation.
pub fn navigate_right(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
    get_text: impl Fn(usize) -> String,
) -> Option<DocumentPosition> {
    let text = get_text(focus.paragraph_index);
    if focus.byte_offset < text.len() {
        let new_offset = next_grapheme_boundary(&text, focus.byte_offset);
        return Some(DocumentPosition { byte_offset: new_offset, ..focus.clone() });
    }

    // At end of paragraph — move to start of next block on the same page.
    let next_index = focus.paragraph_index + 1;
    // Verify the next block is actually on this page.
    find_para_data(layout, focus.page_index, next_index)?;
    // TODO(3b-3): cross-page rightward navigation
    Some(DocumentPosition {
        page_index: focus.page_index,
        paragraph_index: next_index,
        byte_offset: 0,
    })
}

/// Move the cursor one line up, preserving the horizontal screen position.
///
/// Uses `cursor_rect` to find the current line's geometry, then subtracts one
/// line height and re-hits the layout.
///
/// Returns `None` when the cursor is on the first line of the first paragraph
/// on the page (no upward target exists on this page).
///
/// `TODO(3b-3)`: navigate to the last line of the previous page when at the
/// top of the current page.
pub fn navigate_up(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index)?;
    let rect = para_data.layout.cursor_rect(focus.byte_offset)?;
    let margins = &layout.pages.get(focus.page_index)?.margins;

    // Convert paragraph-local → page-canvas-local (what hit_test_page expects).
    let canvas_x = rect.x + para_data.origin.0 + margins.left;
    let canvas_y = rect.y + para_data.origin.1 + margins.top;

    // Aim for the vertical centre of the line above.
    let target_y = canvas_y - rect.height;

    // TODO(3b-3): if target_y < margins.top, navigate to previous page
    hit_test_page(focus.page_index, canvas_x, target_y, layout)
}

/// Move the cursor one line down, preserving the horizontal screen position.
///
/// Returns `None` when the cursor is on the last line of the last paragraph
/// on the page.
///
/// `TODO(3b-3)`: navigate to the first line of the next page when at the
/// bottom of the current page.
pub fn navigate_down(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index)?;
    let rect = para_data.layout.cursor_rect(focus.byte_offset)?;
    let margins = &layout.pages.get(focus.page_index)?.margins;

    let canvas_x = rect.x + para_data.origin.0 + margins.left;
    let canvas_y = rect.y + para_data.origin.1 + margins.top;

    // Aim for the vertical centre of the line below (1.5 × line height down).
    let target_y = canvas_y + rect.height * 1.5;

    // TODO(3b-3): if hit_test_page returns None here, navigate to next page
    hit_test_page(focus.page_index, canvas_x, target_y, layout)
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
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index)?;
    let rect = para_data.layout.cursor_rect(focus.byte_offset)?;

    // Centre-y of the current line in paragraph-local coordinates.
    let line_center_y = rect.y + rect.height / 2.0;
    let hit = para_data.layout.hit_test_point(0.0, line_center_y)?;
    Some(DocumentPosition { byte_offset: hit.byte_offset, ..focus.clone() })
}

/// Move the cursor to the end of the current visual line.
///
/// Uses `cursor_rect` to determine the line's y position, then hit-tests at
/// a very large x (100,000 pt) on the same line — Parley clamps to the last
/// cluster on the line.
///
/// `TODO(3b-3)`: use Parley `line_boundaries` for true End behaviour with
/// bidirectional text.
pub fn navigate_end(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index)?;
    let rect = para_data.layout.cursor_rect(focus.byte_offset)?;

    let line_center_y = rect.y + rect.height / 2.0;
    // 100_000 pt ≈ 1.4 m — well past any realistic page width.
    let hit = para_data.layout.hit_test_point(100_000.0, line_center_y)?;
    Some(DocumentPosition { byte_offset: hit.byte_offset, ..focus.clone() })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loki_layout::{
        layout_paragraph, FontResources, LayoutColor, LayoutInsets, LayoutPage, LayoutSize,
        PaginatedLayout, PageEditingData, PageParagraphData, ResolvedParaProps, StyleSpan,
    };

    use super::*;

    fn make_layout_with_text(text: &str) -> PaginatedLayout {
        let mut resources = FontResources::new();
        let para = layout_paragraph(
            &mut resources,
            text,
            &[StyleSpan {
                range: 0..text.len(),
                font_name: None,
                font_size: 12.0,
                bold: false,
                italic: false,
                color: LayoutColor::BLACK,
                underline: None,
                strikethrough: None,
                line_height: None,
                vertical_align: None,
                highlight_color: None,
                letter_spacing: None,
                font_variant: None,
                word_spacing: None,
                shadow: false,
                link_url: None,
            }],
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        );
        let editing_data = PageEditingData {
            paragraphs: vec![PageParagraphData {
                block_index: 0,
                layout: Arc::new(para),
                origin: (0.0, 0.0),
            }],
        };
        let page_size = LayoutSize::new(595.0, 842.0);
        let margins = LayoutInsets { top: 72.0, right: 72.0, bottom: 72.0, left: 72.0 };
        let page = LayoutPage {
            page_number: 1,
            page_size,
            margins,
            content_items: vec![],
            header_items: vec![],
            footer_items: vec![],
            header_height: 0.0,
            footer_height: 0.0,
            editing_data: Some(editing_data),
        };
        PaginatedLayout { page_size, pages: vec![page] }
    }

    fn focus_at(byte_offset: usize) -> DocumentPosition {
        DocumentPosition { page_index: 0, paragraph_index: 0, byte_offset }
    }

    #[test]
    fn navigate_left_moves_to_prev_grapheme() {
        let layout = make_layout_with_text("hello");
        let focus = focus_at(3);
        let result = navigate_left(&focus, &layout, |_| "hello".to_string());
        assert_eq!(result.unwrap().byte_offset, 2);
    }

    #[test]
    fn navigate_left_at_start_returns_none_for_first_block() {
        let layout = make_layout_with_text("hello");
        let focus = focus_at(0);
        let result = navigate_left(&focus, &layout, |_| "hello".to_string());
        assert!(result.is_none(), "at start of block 0 should return None");
    }

    #[test]
    fn navigate_right_moves_to_next_grapheme() {
        let layout = make_layout_with_text("hello");
        let focus = focus_at(2);
        let result = navigate_right(&focus, &layout, |_| "hello".to_string());
        assert_eq!(result.unwrap().byte_offset, 3);
    }

    #[test]
    fn navigate_right_at_end_of_last_block_returns_none() {
        let layout = make_layout_with_text("hello");
        let focus = focus_at(5); // end of "hello"
        let result = navigate_right(&focus, &layout, |_| "hello".to_string());
        assert!(result.is_none(), "at end of last block should return None");
    }

    #[test]
    fn navigate_home_returns_position_on_same_paragraph() {
        let layout = make_layout_with_text("hello world");
        let focus = focus_at(6);
        let result = navigate_home(&focus, &layout);
        // Home from mid-paragraph — offset ≤ 6 (start of line or paragraph).
        let pos = result.expect("navigate_home should return Some");
        assert_eq!(pos.page_index, 0);
        assert_eq!(pos.paragraph_index, 0);
        assert!(pos.byte_offset <= 6, "Home should move to start of line (byte ≤ 6)");
    }

    #[test]
    fn navigate_end_returns_position_on_same_paragraph() {
        let layout = make_layout_with_text("hello world");
        let focus = focus_at(0);
        let result = navigate_end(&focus, &layout);
        let pos = result.expect("navigate_end should return Some");
        assert_eq!(pos.page_index, 0);
        assert_eq!(pos.paragraph_index, 0);
        // End from start — offset should be > 0.
        assert!(pos.byte_offset > 0, "End should move past the start");
    }

    #[test]
    fn navigate_up_at_first_line_returns_none() {
        let layout = make_layout_with_text("hello");
        let focus = focus_at(0);
        // Single-line paragraph at top of page — no line above.
        let result = navigate_up(&focus, &layout);
        assert!(result.is_none(), "no line above first line should return None");
    }

    #[test]
    fn navigate_down_at_last_line_returns_none() {
        let layout = make_layout_with_text("hello");
        let focus = focus_at(0);
        // Single-line paragraph — no line below.
        let result = navigate_down(&focus, &layout);
        assert!(result.is_none(), "no line below last line should return None");
    }
}
