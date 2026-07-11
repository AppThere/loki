// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `result`.

use super::*;
use crate::color::LayoutColor;
use crate::geometry::{LayoutPoint, LayoutRect};
use crate::items::{GlyphEntry, GlyphSynthesis, PositionedGlyphRun, PositionedRect};
use crate::para::ParagraphLayout;

fn make_filled(x: f32) -> PositionedItem {
    PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(x, 0.0, 10.0, 10.0),
        color: LayoutColor::BLACK,
    })
}

#[test]
fn cell_rotation_forward_inverse_round_trips() {
    // 90° cell, arbitrary pivots. page_to_local ∘ local_to_page == identity.
    let rot = CellRotation {
        degrees: 90.0,
        pivot_local: (30.0, 8.0),
        pivot_page: (100.0, 50.0),
    };
    for (lx, ly) in [(0.0, 0.0), (12.0, 3.0), (30.0, 8.0), (5.0, 16.0)] {
        let (px, py) = rot.local_to_page(lx, ly);
        let (rx, ry) = rot.page_to_local(px, py);
        assert!(
            (rx - lx).abs() < 1e-3 && (ry - ly).abs() < 1e-3,
            "({rx},{ry})"
        );
    }
}

#[test]
fn cell_rotation_90_maps_local_x_to_page_y() {
    // Matches the renderer's Rotate(90°) in y-down coords: local +x → page +y.
    let rot = CellRotation {
        degrees: 90.0,
        pivot_local: (0.0, 0.0),
        pivot_page: (0.0, 0.0),
    };
    let (px, py) = rot.local_to_page(10.0, 0.0);
    assert!(px.abs() < 1e-3 && (py - 10.0).abs() < 1e-3, "({px},{py})");
}

#[test]
fn hit_local_inverts_rotation_for_paragraph() {
    // A rotated paragraph whose content-local origin is (0,0); a page click at
    // the rotated position of local (7, 2) must invert back to paragraph-local
    // (7, 2) so ParagraphLayout::hit_test_point sees the right coordinates.
    let mut p = para("hello", 0, (0.0, 0.0));
    let rot = CellRotation {
        degrees: 270.0,
        pivot_local: (20.0, 5.0),
        pivot_page: (60.0, 40.0),
    };
    p.rotation = Some(rot);
    let (page_x, page_y) = rot.local_to_page(7.0, 2.0);
    let (lx, ly) = p.hit_local(page_x, page_y);
    assert!(
        (lx - 7.0).abs() < 1e-3 && (ly - 2.0).abs() < 1e-3,
        "({lx},{ly})"
    );
}

/// A paragraph whose single glyph run spans local x∈[5, 35] on a baseline at
/// y=10 (font 12 → box y∈[0.4, 12.4]), carrying `url` as its link (or none).
fn link_para(origin: (f32, f32), url: Option<&str>) -> PageParagraphData {
    let run = PositionedGlyphRun {
        origin: LayoutPoint { x: 5.0, y: 10.0 },
        font_data: Arc::new(vec![]),
        font_index: 0,
        font_size: 12.0,
        glyphs: vec![GlyphEntry {
            id: 1,
            x: 0.0,
            y: 0.0,
            advance: 30.0,
        }],
        color: LayoutColor::BLACK,
        synthesis: GlyphSynthesis::default(),
        link_url: url.map(String::from),
    };
    let layout = ParagraphLayout {
        height: 16.0,
        width: 35.0,
        items: vec![PositionedItem::GlyphRun(run)],
        first_baseline: 10.0,
        last_baseline: 10.0,
        line_boundaries: Vec::new(),
        parley_layout: None,
        orig_to_clean: Vec::new(),
        clean_to_orig: Vec::new(),
        indent_start: 0.0,
        indent_hanging: 0.0,
        drop_lines: 0,
        drop_shift: 0.0,
    };
    PageParagraphData {
        block_index: 0,
        path: Vec::new(),
        layout: Arc::new(layout),
        origin,
        rotation: None,
    }
}

#[test]
fn link_at_hits_over_hyperlinked_run() {
    let p = link_para((100.0, 200.0), Some("https://example.com"));
    // Page point over the middle of the run: local (10, 6) ∈ box.
    assert_eq!(p.link_at(110.0, 206.0), Some("https://example.com"));
}

#[test]
fn link_at_misses_outside_the_run_box() {
    let p = link_para((100.0, 200.0), Some("https://example.com"));
    // Right of the run (local x=40 > 35).
    assert_eq!(p.link_at(140.0, 206.0), None);
    // Below the run's box (local y=15 > 12.4) though still inside the paragraph.
    assert_eq!(p.link_at(110.0, 215.0), None);
}

#[test]
fn link_at_none_over_plain_text() {
    let p = link_para((100.0, 200.0), None);
    assert_eq!(p.link_at(110.0, 206.0), None);
}

#[test]
fn link_at_inverts_cell_rotation() {
    // A rotated paragraph: a page click at the rotated position of the run's
    // local centre must still resolve the link.
    let mut p = link_para((0.0, 0.0), Some("https://rot.example"));
    let rot = CellRotation {
        degrees: 90.0,
        pivot_local: (20.0, 5.0),
        pivot_page: (60.0, 40.0),
    };
    p.rotation = Some(rot);
    let (px, py) = rot.local_to_page(10.0, 6.0); // local centre of the run box
    assert_eq!(p.link_at(px, py), Some("https://rot.example"));
}

#[test]
fn continuous_and_page_link_at_delegate_to_paragraphs() {
    let p = link_para((100.0, 200.0), Some("https://both.example"));
    let cont = ContinuousLayout {
        content_width: 500.0,
        total_height: 400.0,
        items: Vec::new(),
        paragraphs: vec![p.clone()],
    };
    assert_eq!(cont.link_at(110.0, 206.0), Some("https://both.example"));
    assert_eq!(cont.link_at(1.0, 1.0), None);

    let page = PageEditingData {
        paragraphs: vec![p],
    };
    assert_eq!(page.link_at(110.0, 206.0), Some("https://both.example"));
    assert_eq!(page.link_at(1.0, 1.0), None);
}

#[test]
fn continuous_all_items_count() {
    let layout = DocumentLayout::Continuous(ContinuousLayout {
        content_width: 500.0,
        total_height: 200.0,
        items: vec![make_filled(0.0), make_filled(20.0), make_filled(40.0)],
        paragraphs: vec![],
    });
    assert_eq!(layout.all_items().count(), 3);
}

fn para(text: &str, block_index: usize, origin: (f32, f32)) -> PageParagraphData {
    use crate::font::FontResources;
    use crate::para::{ResolvedParaProps, StyleSpan, layout_paragraph};
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
        rotation: None,
    }
}

fn two_para_continuous() -> ContinuousLayout {
    let p0 = para("Hello world", 0, (0.0, 0.0));
    let h0 = p0.layout.height;
    let p1 = para("Second line here", 1, (0.0, h0));
    ContinuousLayout {
        content_width: 400.0,
        total_height: h0 + p1.layout.height,
        items: vec![],
        paragraphs: vec![p0, p1],
    }
}

#[test]
fn selection_rects_collapsed_is_empty() {
    let cl = two_para_continuous();
    assert!(cl.selection_rects((0, 3), (0, 3)).is_empty());
}

#[test]
fn selection_rects_within_paragraph() {
    let cl = two_para_continuous();
    let rects = cl.selection_rects((0, 0), (0, 5));
    assert!(!rects.is_empty(), "expected highlight rects");
    // Confined to the first paragraph (origin y = 0, near the top).
    assert!(rects.iter().all(|r| r.origin.y < 30.0));
}

#[test]
fn selection_rects_span_two_paragraphs() {
    let cl = two_para_continuous();
    // Split at the boundary midpoint; line ascent puts a rect's top a point
    // or so above the nominal paragraph origin, so an exact `>= origin`
    // comparison is too strict.
    let mid = cl.paragraphs[1].origin.1 / 2.0;
    let rects = cl.selection_rects((0, 6), (1, 6));
    // Endpoint order is normalised, so reversing gives the same result.
    let rev = cl.selection_rects((1, 6), (0, 6));
    assert_eq!(rects.len(), rev.len());
    assert!(rects.iter().any(|r| r.origin.y < mid)); // first paragraph
    assert!(rects.iter().any(|r| r.origin.y > mid)); // second paragraph
}

#[test]
fn hit_test_and_cursor_round_trip() {
    let cl = two_para_continuous();
    // A click on the second paragraph resolves to block 1.
    let (block, _byte) = cl
        .hit_test(2.0, cl.paragraphs[1].origin.1 + 2.0)
        .expect("hit");
    assert_eq!(block, 1);
    // Caret for the second paragraph sits at/after its canvas origin.
    let cr = cl.cursor_rect_canvas(1, 0).expect("caret");
    assert!(cr.y >= cl.paragraphs[1].origin.1 - 1.0);
}

#[test]
fn paginated_all_items_across_pages() {
    let page1 = LayoutPage {
        page_number: 1,
        page_size: LayoutSize::new(595.0, 842.0),
        margins: LayoutInsets::uniform(72.0),
        content_items: vec![make_filled(0.0), make_filled(10.0)],
        header_items: vec![make_filled(20.0)],
        footer_items: vec![],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    };
    let page2 = LayoutPage {
        page_number: 2,
        page_size: LayoutSize::new(595.0, 842.0),
        margins: LayoutInsets::uniform(72.0),
        content_items: vec![make_filled(0.0)],
        header_items: vec![],
        footer_items: vec![make_filled(30.0)],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    };
    let layout = DocumentLayout::Paginated(PaginatedLayout {
        page_size: LayoutSize::new(595.0, 842.0),
        pages: vec![Arc::new(page1), Arc::new(page2)],
    });
    // page1: 2 content + 1 header = 3; page2: 1 content + 1 footer = 2 → total 5
    assert_eq!(layout.all_items().count(), 5);
}

#[test]
fn layout_page_content_rect() {
    let page = LayoutPage {
        page_number: 1,
        page_size: LayoutSize::new(595.0, 842.0),
        margins: LayoutInsets {
            top: 72.0,
            right: 72.0,
            bottom: 72.0,
            left: 72.0,
        },
        content_items: vec![],
        header_items: vec![],
        footer_items: vec![],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    };
    let cr = page.content_rect();
    assert_eq!(cr.x(), 72.0);
    assert_eq!(cr.y(), 72.0);
    assert_eq!(cr.width(), 595.0 - 144.0);
    assert_eq!(cr.height(), 842.0 - 144.0);
}

#[test]
fn document_layout_total_height_paginated() {
    let make_page = |n: usize| LayoutPage {
        page_number: n,
        page_size: LayoutSize::new(595.0, 842.0),
        margins: LayoutInsets::uniform(72.0),
        content_items: vec![],
        header_items: vec![],
        footer_items: vec![],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    };
    let layout = DocumentLayout::Paginated(PaginatedLayout {
        page_size: LayoutSize::new(595.0, 842.0),
        pages: vec![Arc::new(make_page(1)), Arc::new(make_page(2))],
    });
    assert_eq!(layout.total_height(), 842.0 * 2.0);
}

#[test]
fn document_layout_content_width_continuous() {
    let layout = DocumentLayout::Continuous(ContinuousLayout {
        content_width: 480.0,
        total_height: 100.0,
        items: vec![],
        paragraphs: vec![],
    });
    assert_eq!(layout.content_width(), 480.0);
}
