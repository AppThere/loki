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
const GAP_PX: f32 = 16.0;

fn para(text: &str) -> Arc<ParagraphLayout> {
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
        true,
    ))
}

fn layout_with(paragraph: Arc<ParagraphLayout>) -> PaginatedLayout {
    PaginatedLayout {
        page_size: LayoutSize::new(595.0, PAGE_H),
        pages: vec![Arc::new(LayoutPage {
            page_number: 1,
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
            editing_data: Some(PageEditingData {
                paragraphs: vec![PageParagraphData {
                    block_index: 0,
                    path: Vec::new(),
                    layout: paragraph,
                    origin: (0.0, 0.0),
                    rotation: None,
                }],
            }),
        })],
    }
}

#[test]
fn grab_point_round_trips_through_the_hit_test_transform() {
    let p = para("alpha beta gamma delta");
    let layout = layout_with(Arc::clone(&p));
    let pos = DocumentPosition::top_level(0, 0, 6);
    let origin = (100.0, 50.0);
    let (gx, gy) = handle_grab_point(&layout, &pos, origin, 0.0, PAGE_H, GAP_PX, 1.0)
        .expect("grab point resolves");

    // The grab point sits below the caret's line: hit-testing slightly ABOVE
    // it (back inside the line) must resolve to (approximately) the same
    // byte offset — proving the transform is the hit-test's inverse.
    let back = crate::editing::hit_test::hit_test_document(
        gx,
        gy - (32.0 + 6.0) * (96.0 / 72.0), // undo the teardrop offset + half line
        origin,
        0.0,
        &layout,
        595.0,
        PAGE_H,
        GAP_PX,
        1.0,
    )
    .expect("hit test resolves");
    assert_eq!(back.paragraph_index, 0);
    assert!(
        (back.byte_offset as i64 - 6).abs() <= 1,
        "round trip should land on the same offset, got {}",
        back.byte_offset
    );
}

#[test]
fn grabbing_a_handle_returns_the_opposite_endpoint() {
    let p = para("alpha beta gamma delta");
    let layout = layout_with(Arc::clone(&p));
    let anchor = DocumentPosition::top_level(0, 0, 0);
    let focus = DocumentPosition::top_level(0, 0, 10);
    let origin = (0.0, 0.0);

    let anchor_grab = handle_grab_point(&layout, &anchor, origin, 0.0, PAGE_H, GAP_PX, 1.0)
        .expect("anchor grab point");
    let fixed = grab_fixed_endpoint(
        &layout,
        &anchor,
        &focus,
        anchor_grab,
        origin,
        0.0,
        PAGE_H,
        GAP_PX,
        1.0,
    )
    .expect("grabbing the anchor handle");
    assert_eq!(fixed, focus, "the focus stays fixed");

    // A touch far from both handles grabs nothing.
    assert!(
        grab_fixed_endpoint(
            &layout,
            &anchor,
            &focus,
            (anchor_grab.0 + 500.0, anchor_grab.1 + 500.0),
            origin,
            0.0,
            PAGE_H,
            GAP_PX,
            1.0,
        )
        .is_none()
    );
}

#[test]
fn zoom_scales_the_grab_point() {
    let p = para("alpha beta gamma delta");
    let layout = layout_with(Arc::clone(&p));
    let pos = DocumentPosition::top_level(0, 0, 6);
    let (x1, _) =
        handle_grab_point(&layout, &pos, (0.0, 0.0), 0.0, PAGE_H, GAP_PX, 1.0).expect("1x");
    let (x2, _) =
        handle_grab_point(&layout, &pos, (0.0, 0.0), 0.0, PAGE_H, GAP_PX, 2.0).expect("2x");
    assert!(
        (x2 - 2.0 * x1).abs() < 0.01,
        "grab x must scale with zoom: {x1} vs {x2}"
    );
}
