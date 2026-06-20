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

use loki_layout::{PageParagraphData, PaginatedLayout};

use super::cursor::{DocumentPosition, next_grapheme_boundary, prev_grapheme_boundary};
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
        return Some(DocumentPosition {
            byte_offset: new_offset,
            ..focus.clone()
        });
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
        return Some(DocumentPosition {
            byte_offset: new_offset,
            ..focus.clone()
        });
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
/// line height and re-hits the layout.  When the target falls above the content
/// area (first line of a paragraph or page break), navigates to the last line
/// of the previous paragraph, crossing page boundaries as needed.
///
/// Returns `None` when already at the very first position of the document.
pub fn navigate_up(focus: &DocumentPosition, layout: &PaginatedLayout) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index)?;
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
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index)?;
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
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index)?;
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
/// `get_text(block_index)` is called to retrieve the paragraph text needed by
/// `line_end_offset` to check for a trailing `\n`.
pub fn navigate_end(
    focus: &DocumentPosition,
    layout: &PaginatedLayout,
    get_text: impl Fn(usize) -> String,
) -> Option<DocumentPosition> {
    let para_data = find_para_data(layout, focus.page_index, focus.paragraph_index)?;
    let text = get_text(focus.paragraph_index);
    let end_offset = para_data.layout.line_end_offset(focus.byte_offset, &text)?;
    Some(DocumentPosition {
        byte_offset: end_offset,
        ..focus.clone()
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loki_layout::{
        FontResources, LayoutColor, LayoutInsets, LayoutPage, LayoutSize, PageEditingData,
        PageParagraphData, PaginatedLayout, ResolvedParaProps, StyleSpan, layout_paragraph,
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
        let margins = LayoutInsets {
            top: 72.0,
            right: 72.0,
            bottom: 72.0,
            left: 72.0,
        };
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
        PaginatedLayout {
            page_size,
            pages: vec![Arc::new(page)],
        }
    }

    fn focus_at(byte_offset: usize) -> DocumentPosition {
        DocumentPosition {
            page_index: 0,
            paragraph_index: 0,
            byte_offset,
        }
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
        assert!(
            pos.byte_offset <= 6,
            "Home should move to start of line (byte ≤ 6)"
        );
    }

    #[test]
    fn navigate_end_returns_position_on_same_paragraph() {
        let layout = make_layout_with_text("hello world");
        let focus = focus_at(0);
        let result = navigate_end(&focus, &layout, |_| "hello world".to_string());
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
        assert!(
            result.is_none(),
            "no line above first line should return None"
        );
    }

    #[test]
    fn navigate_down_at_last_line_returns_none() {
        let layout = make_layout_with_text("hello");
        let focus = focus_at(0);
        // Single-line paragraph — no line below.
        let result = navigate_down(&focus, &layout);
        assert!(
            result.is_none(),
            "no line below last line should return None"
        );
    }

    // ── Cross-boundary helper tests ───────────────────────────────────────────

    fn make_two_para_layout(text0: &str, text1: &str) -> PaginatedLayout {
        let mut resources = FontResources::new();
        let make_span = |text: &str| StyleSpan {
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
        };
        let para0 = layout_paragraph(
            &mut resources,
            text0,
            &[make_span(text0)],
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        );
        let h0 = para0.height;
        let para1 = layout_paragraph(
            &mut resources,
            text1,
            &[make_span(text1)],
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        );
        let editing_data = PageEditingData {
            paragraphs: vec![
                PageParagraphData {
                    block_index: 0,
                    layout: Arc::new(para0),
                    origin: (0.0, 0.0),
                },
                PageParagraphData {
                    block_index: 1,
                    layout: Arc::new(para1),
                    origin: (0.0, h0),
                },
            ],
        };
        let page_size = LayoutSize::new(595.0, 842.0);
        let margins = LayoutInsets {
            top: 72.0,
            right: 72.0,
            bottom: 72.0,
            left: 72.0,
        };
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
        PaginatedLayout {
            page_size,
            pages: vec![Arc::new(page)],
        }
    }

    #[test]
    fn find_prev_para_data_at_block_0_returns_none() {
        let layout = make_two_para_layout("first", "second");
        let result = find_prev_para_data(&layout, 0, 0);
        assert!(result.is_none(), "block 0 has no predecessor");
    }

    #[test]
    fn find_prev_para_data_at_block_1_returns_block_0() {
        let layout = make_two_para_layout("first", "second");
        let result = find_prev_para_data(&layout, 0, 1);
        let (page_idx, para) = result.expect("block 1 should have a predecessor");
        assert_eq!(page_idx, 0);
        assert_eq!(para.block_index, 0);
    }

    #[test]
    fn find_next_para_data_at_last_block_returns_none() {
        let layout = make_two_para_layout("first", "second");
        // block_index 1 is the last block; no successor.
        let result = find_next_para_data(&layout, 0, 1);
        assert!(result.is_none(), "last block has no successor");
    }

    #[test]
    fn find_next_para_data_at_block_0_returns_block_1() {
        let layout = make_two_para_layout("first", "second");
        let result = find_next_para_data(&layout, 0, 0);
        let (page_idx, para) = result.expect("block 0 should have a successor");
        assert_eq!(page_idx, 0);
        assert_eq!(para.block_index, 1);
    }
}
