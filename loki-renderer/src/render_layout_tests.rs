// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `render_layout`.

use super::*;
use loki_layout::ContinuousLayout;

fn reflow(total_height: f32, tile_width_pt: f32) -> RenderLayout {
    RenderLayout::Reflow {
        layout: ContinuousLayout {
            content_width: 0.0,
            total_height,
            items: vec![],
            paragraphs: vec![],
        },
        tile_width_pt,
    }
}

#[test]
fn reflow_tile_count_and_sizes() {
    // 768 * 2 + 100 = 1636 → 3 tiles (two full, one 100pt remainder).
    let rl = reflow(1636.0, 500.0);
    assert_eq!(rl.page_count(), 3);
    assert_eq!(rl.page_size_pts(0), Some((500.0, 768.0)));
    assert_eq!(rl.page_size_pts(1), Some((500.0, 768.0)));
    assert_eq!(rl.page_size_pts(2), Some((500.0, 100.0)));
    assert_eq!(rl.page_size_pts(3), None);
    assert!(rl.is_reflow());
    assert!(rl.as_paginated().is_none());
}

#[test]
fn reflow_always_has_at_least_one_tile() {
    let rl = reflow(0.0, 400.0);
    assert_eq!(rl.page_count(), 1);
    assert_eq!(rl.page_size_pts(0), Some((400.0, 1.0)));
}

fn one_para_reflow(text: &str, origin: (f32, f32)) -> RenderLayout {
    use loki_layout::{
        FontResources, LayoutColor, PageParagraphData, ResolvedParaProps, StyleSpan,
        layout_paragraph,
    };
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
    let height = para.height;
    RenderLayout::Reflow {
        layout: ContinuousLayout {
            content_width: 400.0,
            total_height: origin.1 + height,
            items: vec![],
            paragraphs: vec![PageParagraphData {
                block_index: 3,
                path: vec![],
                layout: std::sync::Arc::new(para),
                origin,
            }],
        },
        tile_width_pt: 436.0,
    }
}

#[test]
fn reflow_hit_test_resolves_paragraph_and_offset() {
    let rl = one_para_reflow("Hello world", (5.0, 10.0));
    // Click inside the paragraph: returns its block_index and a byte offset.
    let (block, byte) = rl.reflow_hit_test(8.0, 12.0).expect("hit");
    assert_eq!(block, 3);
    assert!(byte <= "Hello world".len());
    // Far past the end of the line maps to the last offset.
    let (_, byte_end) = rl.reflow_hit_test(390.0, 12.0).expect("hit end");
    assert_eq!(byte_end, "Hello world".len());
    // Paginated layouts have no reflow hit-testing.
    assert_eq!(reflow(100.0, 400.0).reflow_hit_test(8.0, 12.0), None);
}

#[test]
fn reflow_caret_is_offset_by_paragraph_origin() {
    let rl = one_para_reflow("Hello world", (5.0, 40.0));
    let cr = rl.reflow_cursor_canvas(3, 0).expect("caret at start");
    // Caret at byte 0 sits at the paragraph's canvas origin (x≈5, y≈40).
    assert!((cr.x - 5.0).abs() < 2.0, "x={}", cr.x);
    assert!((cr.y - 40.0).abs() < 4.0, "y={}", cr.y);
    assert!(cr.height > 0.0);
    // Unknown paragraph → None.
    assert!(rl.reflow_cursor_canvas(99, 0).is_none());
}

#[test]
fn render_mode_width_tolerant_equality() {
    let a = RenderMode::Reflow {
        available_width_pt: 600.0,
    };
    let b = RenderMode::Reflow {
        available_width_pt: 600.3,
    };
    let c = RenderMode::Reflow {
        available_width_pt: 620.0,
    };
    assert!(a.matches(&b));
    assert!(!a.matches(&c));
    assert!(!a.matches(&RenderMode::Paginated));
}

// ── Spec 03 M4: bounded reflow measure ──────────────────────────────────────

#[test]
fn narrow_viewport_uses_its_full_width() {
    // Below the cap the tile tracks the viewport (phones use the whole screen).
    assert_eq!(reflow_tile_width_px(375.0), 375.0);
    assert_eq!(reflow_tile_width_px(600.0), 600.0);
}

#[test]
fn wide_viewport_caps_the_measure_so_it_can_centre() {
    // At and beyond the cap the tile stops growing — leaving room for the
    // renderer's `margin: auto` to centre the reading column.
    assert_eq!(reflow_tile_width_px(MAX_REFLOW_TILE_PX), MAX_REFLOW_TILE_PX);
    assert_eq!(reflow_tile_width_px(2560.0), MAX_REFLOW_TILE_PX);
    // The measure no longer grows with the window — the "cramped"/edge-to-edge
    // bug (R-6) cannot recur on a wide window.
    assert_eq!(
        reflow_content_width_pt(1600.0),
        reflow_content_width_pt(3000.0)
    );
}

#[test]
fn content_width_is_tile_minus_insets_and_floored() {
    // Content = tile(px)·PX_TO_PT − 2·padding, in points.
    let expect = MAX_REFLOW_TILE_PX * PX_TO_PT - 2.0 * REFLOW_PADDING_PT;
    assert!((reflow_content_width_pt(2000.0) - expect).abs() < 1e-3);
    // A degenerate (tiny) viewport floors at the engine minimum, never negative.
    assert_eq!(reflow_content_width_pt(1.0), MIN_REFLOW_CONTENT_PT);
    assert_eq!(reflow_content_width_pt(0.0), MIN_REFLOW_CONTENT_PT);
}
