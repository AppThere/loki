// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::sync::Arc;

use loki_layout::{
    FontResources, LayoutColor, LayoutInsets, LayoutPage, LayoutSize, PageEditingData,
    PageParagraphData, PaginatedLayout, ResolvedParaProps, StyleSpan, layout_paragraph,
};

use super::*;

fn layout_para(
    resources: &mut FontResources,
    text: &str,
) -> std::sync::Arc<loki_layout::ParagraphLayout> {
    Arc::new(layout_paragraph(
        resources,
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
    ))
}

/// One page whose editing data holds the given `(block_index, path, text)`
/// paragraphs, stacked vertically.
fn make_layout_with_paras(paras: &[(usize, Vec<PathStep>, &str)]) -> PaginatedLayout {
    let mut resources = FontResources::new();
    let mut y = 0.0f32;
    let mut entries = Vec::new();
    for (block_index, path, text) in paras {
        let layout = layout_para(&mut resources, text);
        let h = layout.height;
        entries.push(PageParagraphData {
            block_index: *block_index,
            path: path.clone(),
            layout,
            origin: (0.0, y),
        });
        y += h;
    }
    let editing_data = PageEditingData {
        paragraphs: entries,
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
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: Some(editing_data),
    };
    PaginatedLayout {
        page_size,
        pages: vec![Arc::new(page)],
    }
}

fn make_layout_with_text(text: &str) -> PaginatedLayout {
    make_layout_with_paras(&[(0, Vec::new(), text)])
}

fn focus_at(byte_offset: usize) -> DocumentPosition {
    DocumentPosition::top_level(0, 0, byte_offset)
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
                path: Vec::new(),
                layout: Arc::new(para0),
                origin: (0.0, 0.0),
            },
            PageParagraphData {
                block_index: 1,
                path: Vec::new(),
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
        comment_items: vec![],
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

// ── Nested (cell / note body) sibling navigation (4b.4) ───────────────────────

/// `[Para "intro", Table]` where the table (block 1) has one cell holding the
/// paragraphs "alpha" | "beta", followed by a top-level "outro" (block 2).
fn nested_cell_layout() -> PaginatedLayout {
    make_layout_with_paras(&[
        (0, Vec::new(), "intro"),
        (1, vec![PathStep::Cell { cell: 0, block: 0 }], "alpha"),
        (1, vec![PathStep::Cell { cell: 0, block: 1 }], "beta"),
        (2, Vec::new(), "outro"),
    ])
}

fn nested_text(bp: &BlockPath) -> String {
    match (bp.root, bp.steps.as_slice()) {
        (0, []) => "intro",
        (1, [PathStep::Cell { cell: 0, block: 0 }]) => "alpha",
        (1, [PathStep::Cell { cell: 0, block: 1 }]) => "beta",
        (2, []) => "outro",
        _ => "",
    }
    .to_string()
}

fn cell_pos(block: usize, byte_offset: usize) -> DocumentPosition {
    DocumentPosition {
        page_index: 0,
        paragraph_index: 1,
        byte_offset,
        path: vec![PathStep::Cell { cell: 0, block }],
    }
}

#[test]
fn navigate_right_crosses_to_the_next_cell_sibling() {
    let layout = nested_cell_layout();
    // End of "alpha" → start of "beta", keeping the cell path.
    let result = navigate_right(&cell_pos(0, 5), &layout, nested_text);
    assert_eq!(result, Some(cell_pos(1, 0)));
}

#[test]
fn navigate_left_crosses_to_the_previous_cell_sibling() {
    let layout = nested_cell_layout();
    // Start of "beta" → end of "alpha" (byte 5), keeping the cell path.
    let result = navigate_left(&cell_pos(1, 0), &layout, nested_text);
    assert_eq!(result, Some(cell_pos(0, 5)));
}

#[test]
fn navigate_left_clamps_at_the_first_block_of_a_cell() {
    let layout = nested_cell_layout();
    // Start of "alpha": must NOT teleport to the top-level "intro" paragraph.
    assert_eq!(navigate_left(&cell_pos(0, 0), &layout, nested_text), None);
}

#[test]
fn navigate_right_clamps_at_the_last_block_of_a_cell() {
    let layout = nested_cell_layout();
    // End of "beta": must NOT teleport to the top-level "outro" paragraph.
    assert_eq!(navigate_right(&cell_pos(1, 4), &layout, nested_text), None);
}

#[test]
fn grapheme_navigation_inside_a_cell_uses_the_cell_text() {
    let layout = nested_cell_layout();
    // Mid-"alpha" moves within the nested paragraph's own text (previously
    // the flat getter returned the root table block's empty text).
    let result = navigate_right(&cell_pos(0, 2), &layout, nested_text);
    assert_eq!(result, Some(cell_pos(0, 3)));
    let result = navigate_left(&cell_pos(0, 2), &layout, nested_text);
    assert_eq!(result, Some(cell_pos(0, 1)));
}

#[test]
fn navigate_end_inside_a_cell_uses_the_cell_line() {
    let layout = nested_cell_layout();
    let result = navigate_end(&cell_pos(0, 1), &layout, nested_text);
    assert_eq!(result, Some(cell_pos(0, 5)));
}
