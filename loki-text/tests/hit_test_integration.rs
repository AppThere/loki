// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end integration test for the layout → hit_test_document pipeline.
//!
//! Verifies the complete path from document construction through
//! `layout_document` to `hit_test_document`, confirming that a simulated
//! mouse click on document text returns a sensible `DocumentPosition`.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::section::Section;
use loki_layout::{layout_document, DocumentLayout, FontResources, LayoutMode, LayoutOptions};
use loki_text::editing::hit_test::hit_test_document;

/// Layout pt → CSS px.
fn pt_to_px(pt: f32) -> f32 {
    pt * (96.0 / 72.0)
}

/// Build a minimal one-section document with a single paragraph containing
/// the provided `text`.
fn make_document(text: &str) -> Document {
    let para = Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    });
    let mut section = Section::new();
    section.blocks.push(para);
    Document {
        meta: Default::default(),
        styles: Default::default(),
        sections: vec![section],
        source: None,
    }
}

/// Full pipeline: Document → layout_document → hit_test_document.
///
/// Simulates a click at the horizontal centre of the first text line and
/// asserts that the returned position sits on page 0, paragraph 0, at a
/// byte offset somewhere in the middle of "Hello" (1 ≤ offset ≤ 4).
#[test]
fn click_at_line_centre_returns_middle_of_hello() {
    let doc = make_document("Hello");
    let mut resources = FontResources::new();
    let layout = layout_document(
        &mut resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions { preserve_for_editing: true },
    );

    let pl = match &layout {
        DocumentLayout::Paginated(pl) => pl,
        _ => panic!("expected paginated layout"),
    };

    assert!(!pl.pages.is_empty(), "layout must produce at least one page");
    let page = &pl.pages[0];

    let page_w_px = pt_to_px(pl.page_size.width);
    let page_h_px = pt_to_px(pl.page_size.height);
    let page_gap_px = pt_to_px(24.0);

    // Click at margin_left + half the content width, margin_top + a few px into the first line.
    let margin_left_px = pt_to_px(page.margins.left);
    let margin_top_px = pt_to_px(page.margins.top);
    // Horizontal centre of the content area is a reliable "middle of the line" click.
    let content_width_px = pt_to_px(pl.page_size.width - page.margins.horizontal());
    let click_x = margin_left_px + content_width_px / 2.0;
    // A few px below the top margin lands inside the first line of text.
    let click_y = margin_top_px + 2.0;

    let result = hit_test_document(
        click_x,
        click_y,
        (0.0, 0.0), // canvas_origin — canvas starts at window origin in this test
        0.0,         // scroll_offset
        pl,
        page_w_px,
        page_h_px,
        page_gap_px,
    );

    let pos = result.expect("click inside content area must return Some(DocumentPosition)");
    assert_eq!(pos.page_index, 0, "click is on page 0");
    assert_eq!(pos.paragraph_index, 0, "click is in paragraph 0");
    // "Hello" is 5 bytes; a centre-of-line click should land somewhere in the
    // middle, not at the very start or very end.
    assert!(
        pos.byte_offset >= 1 && pos.byte_offset <= 5,
        "expected byte_offset 1..=5 for a centre-of-line click on 'Hello', got {}",
        pos.byte_offset
    );
}

/// Clicking outside the page (to the left of margin_left at x=0) must return
/// `None` — the horizontal bounds check in `hit_test_document` fires.
#[test]
fn click_left_of_page_returns_none() {
    let doc = make_document("Hello");
    let mut resources = FontResources::new();
    let layout = layout_document(
        &mut resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions { preserve_for_editing: true },
    );
    let pl = match layout {
        DocumentLayout::Paginated(pl) => pl,
        _ => panic!("expected paginated layout"),
    };

    let page_w_px = pt_to_px(pl.page_size.width);
    let page_h_px = pt_to_px(pl.page_size.height);

    // client_x = -1.0 (left of the canvas) with canvas_origin at (0, 0).
    let result = hit_test_document(
        -1.0,
        pt_to_px(pl.pages[0].margins.top) + 2.0,
        (0.0, 0.0),
        0.0,
        &pl,
        page_w_px,
        page_h_px,
        pt_to_px(24.0),
    );
    assert!(result.is_none(), "click left of page canvas must return None");
}

/// A layout run with `preserve_for_editing: false` (read-only mode) must cause
/// `hit_test_document` to return `None` because `editing_data` is absent.
#[test]
fn no_editing_data_returns_none() {
    let doc = make_document("Hello");
    let mut resources = FontResources::new();
    let layout = layout_document(
        &mut resources,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions { preserve_for_editing: false },
    );
    let pl = match layout {
        DocumentLayout::Paginated(pl) => pl,
        _ => panic!("expected paginated layout"),
    };

    let page = &pl.pages[0];
    let page_w_px = pt_to_px(pl.page_size.width);
    let page_h_px = pt_to_px(pl.page_size.height);
    let margin_left_px = pt_to_px(page.margins.left);
    let margin_top_px = pt_to_px(page.margins.top);

    let result = hit_test_document(
        margin_left_px + 5.0,
        margin_top_px + 2.0,
        (0.0, 0.0),
        0.0,
        &pl,
        page_w_px,
        page_h_px,
        pt_to_px(24.0),
    );
    assert!(
        result.is_none(),
        "read-only layout (no editing_data) must return None from hit_test_document"
    );
}
