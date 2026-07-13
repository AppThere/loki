// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::sync::Arc;

use loki_layout::{
    FontResources, LayoutColor, LayoutInsets, LayoutPage, LayoutSize, PageEditingData,
    PageParagraphData, PaginatedLayout, ParagraphLayout, ResolvedParaProps, StyleSpan,
    layout_paragraph,
};

use super::*;

const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 72.0;
const CONTENT_H: f32 = PAGE_H - 2.0 * MARGIN;

fn para(text: &str, width: f32) -> Arc<ParagraphLayout> {
    let mut resources = FontResources::new();
    Arc::new(layout_paragraph(
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
            word_spacing: None,
            font_variant: None,
            shadow: false,
            link_url: None,
            math: None,
            scale: None,
            kerning: None,
            baseline_shift: None,
            language: None,
        }],
        &ResolvedParaProps::default(),
        width,
        1.0,
        true,
    ))
}

fn page(paragraphs: Vec<PageParagraphData>, number: usize) -> Arc<LayoutPage> {
    Arc::new(LayoutPage {
        page_number: number,
        page_size: LayoutSize::new(595.0, PAGE_H),
        margins: LayoutInsets {
            top: MARGIN,
            right: MARGIN,
            bottom: MARGIN,
            left: MARGIN,
        },
        content_items: vec![],
        header_items: vec![],
        footer_items: vec![],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: Some(PageEditingData { paragraphs }),
    })
}

fn entry(block_index: usize, layout: Arc<ParagraphLayout>, y: f32) -> PageParagraphData {
    PageParagraphData {
        block_index,
        path: Vec::new(),
        layout,
        origin: (0.0, y),
        rotation: None,
    }
}

#[test]
fn moves_the_page_index_to_the_paragraphs_page() {
    // Block 0 on page 0, block 1 on page 1 — a position for block 1 that
    // still carries page 0 (e.g. after a split pushed it over) is corrected.
    let p0 = para("first", 400.0);
    let p1 = para("second", 400.0);
    let layout = PaginatedLayout {
        page_size: LayoutSize::new(595.0, PAGE_H),
        pages: vec![
            page(vec![entry(0, p0, 0.0)], 1),
            page(vec![entry(1, p1, 0.0)], 2),
        ],
    };
    let stale = DocumentPosition::top_level(0, 1, 0);
    let fixed = recompute_page_index(&layout, &stale);
    assert_eq!(fixed.page_index, 1);
    assert_eq!(fixed.paragraph_index, 1);
}

#[test]
fn keeps_the_page_index_when_already_correct() {
    let p0 = para("only", 400.0);
    let layout = PaginatedLayout {
        page_size: LayoutSize::new(595.0, PAGE_H),
        pages: vec![page(vec![entry(0, p0, 0.0)], 1)],
    };
    let pos = DocumentPosition::top_level(0, 0, 2);
    assert_eq!(recompute_page_index(&layout, &pos).page_index, 0);
}

#[test]
fn unknown_paragraph_leaves_the_position_unchanged() {
    let p0 = para("only", 400.0);
    let layout = PaginatedLayout {
        page_size: LayoutSize::new(595.0, PAGE_H),
        pages: vec![page(vec![entry(0, p0, 0.0)], 1)],
    };
    let pos = DocumentPosition::top_level(0, 9, 0);
    assert_eq!(recompute_page_index(&layout, &pos), pos);
}

#[test]
fn split_paragraph_picks_the_page_showing_the_bytes_line() {
    // One long paragraph wrapped to many lines at a narrow measure, split
    // across two pages the way the flow engine does it: the same layout on
    // both pages, page 1's fragment shifted up so its visible band starts
    // where page 0's ended.
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
    let p = para(text, 60.0); // narrow → many lines
    let total_h = p.height;
    assert!(
        total_h > 40.0,
        "test premise: the paragraph wraps to several lines (height {total_h})"
    );
    // Page 0 shows the first half, page 1 the rest.
    let cut = total_h / 2.0;
    let layout = PaginatedLayout {
        page_size: LayoutSize::new(595.0, PAGE_H),
        pages: vec![
            // Fragment on page 0 ends its visible band at `cut`: place it so
            // the paragraph starts at the bottom of the content area minus cut.
            page(vec![entry(0, Arc::clone(&p), CONTENT_H - cut)], 1),
            // Fragment on page 1 starts `cut` into the paragraph.
            page(vec![entry(0, Arc::clone(&p), -cut)], 2),
        ],
    };

    // Byte 0 (first line) is visible on page 0 only.
    let first = recompute_page_index(&layout, &DocumentPosition::top_level(1, 0, 0));
    assert_eq!(first.page_index, 0);

    // The last byte (last line) is visible on page 1 only.
    let last = recompute_page_index(&layout, &DocumentPosition::top_level(0, 0, text.len()));
    assert_eq!(last.page_index, 1);
}

#[test]
fn first_line_after_a_page_break_resolves_to_the_new_page() {
    // Regression: the split engine moves a line that does not fit page 0
    // entirely onto page 1, leaving up to a line of slack at page 0's bottom.
    // With more than half a line of slack, the old line-CENTRE band check
    // still claimed the line for page 0 and the caret painted on the previous
    // page. The line must resolve to the page that actually renders it.
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
    let p = para(text, 60.0); // narrow → many lines
    // Find a mid-paragraph line boundary: the line top of the middle byte.
    let mid = text.len() / 2;
    let mid_rect = p.cursor_rect(mid).expect("mid rect");
    let line_top = mid_rect.y;
    let line_h = mid_rect.height;
    assert!(line_top > 0.0, "test premise: mid byte is not on line 0");

    // Page 0 shows lines [0, line_top) and then 80% of a line of slack —
    // the mid line itself did NOT fit and went to page 1.
    let slack = 0.8 * line_h;
    let layout = PaginatedLayout {
        page_size: LayoutSize::new(595.0, PAGE_H),
        pages: vec![
            page(
                vec![entry(0, Arc::clone(&p), CONTENT_H - line_top - slack)],
                1,
            ),
            page(vec![entry(0, Arc::clone(&p), -line_top)], 2),
        ],
    };

    // A caret on the moved line must paint on page 1, not page 0.
    let fixed = recompute_page_index(&layout, &DocumentPosition::top_level(0, 0, mid));
    assert_eq!(
        fixed.page_index, 1,
        "the first line after the break renders on page 1"
    );

    // And a caret on page 0's own last line stays on page 0.
    let first = recompute_page_index(&layout, &DocumentPosition::top_level(1, 0, 0));
    assert_eq!(first.page_index, 0);
}
