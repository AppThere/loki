// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cursor navigation for the **reflow** (non-paginated) view.
//!
//! Mirrors [`super::navigation`] but operates on a [`ContinuousLayout`] (the
//! whole document as one canvas) rather than a `PaginatedLayout`.  All geometry
//! is in canvas-absolute layout points.  Returned positions use
//! `page_index = 0` (meaningless in reflow; the renderer derives the band).
//!
//! Returns `None` when the move would leave the document, so the caller keeps
//! the cursor unchanged.

use loki_layout::ContinuousLayout;

use super::cursor::{DocumentPosition, next_grapheme_boundary, prev_grapheme_boundary};

/// Index of `block_index` within the layout's document-ordered paragraph list.
fn para_pos(layout: &ContinuousLayout, block_index: usize) -> Option<usize> {
    layout
        .paragraphs
        .iter()
        .position(|p| p.block_index == block_index)
}

fn pos_at(paragraph_index: usize, byte_offset: usize) -> DocumentPosition {
    DocumentPosition::top_level(0, paragraph_index, byte_offset)
}

/// Move one grapheme left, crossing into the previous paragraph at offset 0.
pub fn reflow_navigate_left(
    focus: &DocumentPosition,
    layout: &ContinuousLayout,
    get_text: impl Fn(usize) -> String,
) -> Option<DocumentPosition> {
    if focus.byte_offset > 0 {
        let text = get_text(focus.paragraph_index);
        let new_offset = prev_grapheme_boundary(&text, focus.byte_offset);
        return Some(pos_at(focus.paragraph_index, new_offset));
    }
    let cur = para_pos(layout, focus.paragraph_index)?;
    if cur == 0 {
        return None;
    }
    let prev = &layout.paragraphs[cur - 1];
    let prev_len = get_text(prev.block_index).len();
    Some(pos_at(prev.block_index, prev_len))
}

/// Move one grapheme right, crossing into the next paragraph at its end.
pub fn reflow_navigate_right(
    focus: &DocumentPosition,
    layout: &ContinuousLayout,
    get_text: impl Fn(usize) -> String,
) -> Option<DocumentPosition> {
    let text = get_text(focus.paragraph_index);
    if focus.byte_offset < text.len() {
        let new_offset = next_grapheme_boundary(&text, focus.byte_offset);
        return Some(pos_at(focus.paragraph_index, new_offset));
    }
    let cur = para_pos(layout, focus.paragraph_index)?;
    let next = layout.paragraphs.get(cur + 1)?;
    Some(pos_at(next.block_index, 0))
}

/// Move one line up, preserving the horizontal position.
pub fn reflow_navigate_up(
    focus: &DocumentPosition,
    layout: &ContinuousLayout,
) -> Option<DocumentPosition> {
    let para = layout.paragraph(focus.paragraph_index)?;
    let rect = para.layout.cursor_rect(focus.byte_offset)?;
    let canvas_x = para.origin.0 + rect.x;
    let canvas_y = para.origin.1 + rect.y;
    let target_y = canvas_y - rect.height;

    if target_y >= 0.0
        && let Some((block, byte)) = layout.hit_test(canvas_x, target_y)
        && (block != focus.paragraph_index || byte != focus.byte_offset)
    {
        return Some(pos_at(block, byte));
    }

    // Cross-paragraph: last line of the previous paragraph at the same x.
    let cur = para_pos(layout, focus.paragraph_index)?;
    if cur == 0 {
        return None;
    }
    let prev = &layout.paragraphs[cur - 1];
    let prev_bottom = prev.origin.1 + prev.layout.height - 0.5;
    layout
        .hit_test(canvas_x, prev_bottom)
        .map(|(b, o)| pos_at(b, o))
}

/// Move one line down, preserving the horizontal position.
pub fn reflow_navigate_down(
    focus: &DocumentPosition,
    layout: &ContinuousLayout,
) -> Option<DocumentPosition> {
    let para = layout.paragraph(focus.paragraph_index)?;
    let rect = para.layout.cursor_rect(focus.byte_offset)?;
    let canvas_x = para.origin.0 + rect.x;
    let canvas_y = para.origin.1 + rect.y;
    let target_y = canvas_y + rect.height * 1.5;

    if target_y < layout.total_height
        && let Some((block, byte)) = layout.hit_test(canvas_x, target_y)
        && (block != focus.paragraph_index || byte != focus.byte_offset)
    {
        return Some(pos_at(block, byte));
    }

    // Cross-paragraph: first line of the next paragraph at the same x.
    let cur = para_pos(layout, focus.paragraph_index)?;
    let next = layout.paragraphs.get(cur + 1)?;
    let next_top = next.origin.1 + 0.5;
    layout
        .hit_test(canvas_x, next_top)
        .map(|(b, o)| pos_at(b, o))
}

/// Move to the start of the current visual line.
pub fn reflow_navigate_home(
    focus: &DocumentPosition,
    layout: &ContinuousLayout,
) -> Option<DocumentPosition> {
    let para = layout.paragraph(focus.paragraph_index)?;
    let rect = para.layout.cursor_rect(focus.byte_offset)?;
    let line_center_y = rect.y + rect.height / 2.0;
    let hit = para.layout.hit_test_point(0.0, line_center_y)?;
    Some(pos_at(focus.paragraph_index, hit.byte_offset))
}

/// Move to the end of the current visual line.
pub fn reflow_navigate_end(
    focus: &DocumentPosition,
    layout: &ContinuousLayout,
    get_text: impl Fn(usize) -> String,
) -> Option<DocumentPosition> {
    let para = layout.paragraph(focus.paragraph_index)?;
    let text = get_text(focus.paragraph_index);
    let end_offset = para.layout.line_end_offset(focus.byte_offset, &text)?;
    Some(pos_at(focus.paragraph_index, end_offset))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loki_layout::{
        ContinuousLayout, FontResources, LayoutColor, PageParagraphData, ResolvedParaProps,
        StyleSpan, layout_paragraph,
    };

    use super::*;

    fn make_para(text: &str, block_index: usize, origin: (f32, f32)) -> PageParagraphData {
        let mut resources = FontResources::new();
        let layout = layout_paragraph(
            &mut resources,
            text,
            &[StyleSpan {
                range: 0..text.len(),
                font_name: None,
                font_size: 12.0,
                bold: false,
                weight: 400,
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
                math: None,
                scale: None,
                kerning: None,
                baseline_shift: None,
            }],
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        );
        PageParagraphData {
            block_index,
            path: Vec::new(),
            layout: Arc::new(layout),
            origin,
        }
    }

    fn two_paras() -> ContinuousLayout {
        let p0 = make_para("first", 0, (0.0, 0.0));
        let h0 = p0.layout.height;
        let p1 = make_para("second", 1, (0.0, h0));
        ContinuousLayout {
            content_width: 400.0,
            total_height: h0 + p1.layout.height,
            items: vec![],
            paragraphs: vec![p0, p1],
        }
    }

    fn pos(p: usize, b: usize) -> DocumentPosition {
        DocumentPosition::top_level(0, p, b)
    }

    #[test]
    fn left_within_then_crosses_paragraph() {
        let cl = two_paras();
        let texts = |i: usize| ["first", "second"][i].to_string();
        // Within paragraph 1.
        assert_eq!(
            reflow_navigate_left(&pos(1, 3), &cl, texts)
                .unwrap()
                .byte_offset,
            2
        );
        // At offset 0 of paragraph 1 → end of paragraph 0 ("first" = 5).
        let crossed = reflow_navigate_left(&pos(1, 0), &cl, texts).unwrap();
        assert_eq!(crossed.paragraph_index, 0);
        assert_eq!(crossed.byte_offset, 5);
        // At the very start → None.
        assert!(reflow_navigate_left(&pos(0, 0), &cl, texts).is_none());
    }

    #[test]
    fn right_crosses_into_next_paragraph() {
        let cl = two_paras();
        let texts = |i: usize| ["first", "second"][i].to_string();
        // End of paragraph 0 → start of paragraph 1.
        let crossed = reflow_navigate_right(&pos(0, 5), &cl, texts).unwrap();
        assert_eq!(crossed.paragraph_index, 1);
        assert_eq!(crossed.byte_offset, 0);
        // End of the last paragraph → None.
        assert!(reflow_navigate_right(&pos(1, 6), &cl, texts).is_none());
    }

    #[test]
    fn down_then_up_move_between_paragraphs() {
        let cl = two_paras();
        // From paragraph 0 (single line), Down lands in paragraph 1.
        let down = reflow_navigate_down(&pos(0, 1), &cl).expect("down");
        assert_eq!(down.paragraph_index, 1);
        // From paragraph 1, Up returns to paragraph 0.
        let up = reflow_navigate_up(&pos(1, 1), &cl).expect("up");
        assert_eq!(up.paragraph_index, 0);
        // Up from the first line of the document → None.
        assert!(reflow_navigate_up(&pos(0, 0), &cl).is_none());
    }
}
