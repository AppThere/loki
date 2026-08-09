// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::sync::Arc;

use loki_layout::{
    ContinuousLayout, CursorRect, FontResources, LayoutColor, PageParagraphData, ResolvedParaProps,
    StyleSpan, layout_paragraph,
};

use super::*;

/// A minimal reflow paragraph fixture, mirroring
/// `crate::editing::hit_test_tests::reflow_para` — built via the real
/// `loki_layout::layout_paragraph` (not hand-rolled geometry), so the
/// fixture's line breaks and heights are the same shaping this view's own
/// hit-testing depends on.
fn reflow_para(text: &str, block_index: usize, origin: (f32, f32)) -> PageParagraphData {
    let mut resources = FontResources::new();
    let para = layout_paragraph(
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
            emboss: false,
            imprint: false,
            character_border: None,
            link_url: None,
            math: None,
            scale: None,
            kerning: None,
            baseline_shift: None,
            language: None,
        }],
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true, // preserve_for_editing — retains the hit-test layout
    );
    PageParagraphData {
        block_index,
        path: Vec::new(),
        layout: Arc::new(para),
        origin,
        rotation: None,
    }
}

fn two_para_continuous() -> ContinuousLayout {
    let p0 = reflow_para("Hello world", 0, (0.0, 0.0));
    let h0 = p0.layout.height;
    let p1 = reflow_para("Second paragraph here", 1, (0.0, h0));
    ContinuousLayout {
        content_width: 400.0,
        total_height: h0 + p1.layout.height,
        items: vec![],
        paragraphs: vec![p0, p1],
    }
}

/// px→pt at the ratio [`resolve_click_position`] applies internally, so
/// tests can state click points in points (matching the fixture's own units)
/// and convert once, the same way the production caller's
/// `element_coordinates()` (CSS px) does.
fn pt_to_px(pt: f32) -> f32 {
    pt / PX_TO_PT
}

#[test]
fn click_in_first_paragraph_resolves_to_block_0() {
    let cl = two_para_continuous();
    let pos = resolve_click_position(&cl, pt_to_px(5.0), pt_to_px(2.0))
        .expect("a click inside the first paragraph resolves");
    assert_eq!(pos.paragraph_index, 0);
}

#[test]
fn click_in_second_paragraph_resolves_to_block_1() {
    let cl = two_para_continuous();
    let h0 = cl.paragraphs[0].layout.height;
    let pos = resolve_click_position(&cl, pt_to_px(5.0), pt_to_px(h0 + 2.0))
        .expect("a click inside the second paragraph resolves");
    assert_eq!(pos.paragraph_index, 1);
}

/// The DOM reflow view has no page concept and does not yet address nested
/// containers (`TODO(dom-reflow-editing)`), so every resolved position must
/// be page 0 with an empty path — asserted explicitly so a future change
/// that starts threading a real page index or path through does not silently
/// disagree with `caret_el`'s `cursor_rect_canvas(paragraph_index,
/// byte_offset)` call, which ignores both.
#[test]
fn resolved_position_is_top_level_with_page_zero() {
    let cl = two_para_continuous();
    let pos = resolve_click_position(&cl, pt_to_px(5.0), pt_to_px(2.0)).expect("resolves");
    assert_eq!(pos.page_index, 0);
    assert!(pos.path.is_empty());
}

#[test]
fn empty_layout_resolves_to_none() {
    let cl = ContinuousLayout {
        content_width: 400.0,
        total_height: 0.0,
        items: vec![],
        paragraphs: vec![],
    };
    assert_eq!(
        resolve_click_position(&cl, pt_to_px(5.0), pt_to_px(2.0)),
        None
    );
}

#[test]
fn caret_css_places_the_line_at_the_rect_and_hides_it_from_hit_testing() {
    let rect = CursorRect {
        x: 12.5,
        y: 30.0,
        height: 14.0,
    };
    let css = caret_css(&rect);
    assert!(css.contains("left: 12.5pt"), "css was: {css}");
    assert!(css.contains("top: 30pt"), "css was: {css}");
    assert!(css.contains("height: 14pt"), "css was: {css}");
    assert!(css.contains("position: absolute"));
    // Without this, the caret itself would swallow the click meant for the
    // text underneath it — `place_caret_at_click` hit-tests the column, not
    // the caret element.
    assert!(css.contains("pointer-events: none"));
}
