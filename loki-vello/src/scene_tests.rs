// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the Vello scene builder (`super`). Extracted from scene.rs (Phase 7.1 inline-test extraction).

use std::sync::Arc;

use loki_layout::{
    GlyphSynthesis, LayoutColor, LayoutPoint, LayoutRect, PositionedGlyphRun, PositionedItem,
    PositionedRect,
};

use super::items::translate_item;
use super::*;
use crate::font_cache::FontDataCache;

fn make_page(page_number: usize, w: f32, h: f32) -> LayoutPage {
    use loki_layout::{LayoutInsets, LayoutSize};
    LayoutPage {
        page_number,
        page_size: LayoutSize::new(w, h),
        margins: LayoutInsets::uniform(72.0),
        content_items: vec![],
        header_items: vec![],
        footer_items: vec![],
        comment_items: vec![],
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    }
}

// The page chrome (white background + drop shadow) must be sized from each
// page's own size, not the document-level default. Regression guard for the
// gray-streak bug on A4 / landscape pages whose size differs from page 1.
#[test]
fn page_chrome_uses_per_page_size() {
    use loki_layout::LayoutSize;
    let letter = make_page(1, 612.0, 792.0);
    let a4_landscape = make_page(2, 842.0, 595.0);
    assert_eq!(page_chrome_size(&letter), (612.0, 792.0));
    // Even though a hypothetical document default differs, the second page
    // reports its own (wider, shorter) landscape size.
    assert_eq!(page_chrome_size(&a4_landscape), (842.0, 595.0));
    // Sanity: helper reads page_size, not a constant.
    let custom = make_page(3, LayoutSize::new(100.0, 200.0).width, 200.0);
    assert_eq!(page_chrome_size(&custom), (100.0, 200.0));
}

fn make_continuous_layout(items: Vec<PositionedItem>) -> DocumentLayout {
    DocumentLayout::Continuous(ContinuousLayout {
        content_width: 400.0,
        total_height: 100.0,
        items,
        paragraphs: vec![],
    })
}

#[test]
fn paint_decoration_every_style_does_not_panic() {
    use loki_layout::items::DecorationStyle;
    use loki_layout::{DecorationKind, PositionedDecoration};
    let styles = [
        DecorationStyle::Solid,
        DecorationStyle::Double,
        DecorationStyle::Dotted,
        DecorationStyle::Dashed,
        DecorationStyle::Wave,
        DecorationStyle::Thick,
    ];
    for kind in [DecorationKind::Underline, DecorationKind::Strikethrough] {
        for style in styles {
            let mut scene = vello::Scene::new();
            let deco = PositionedDecoration {
                x: 10.0,
                y: 20.0,
                width: 80.0,
                thickness: 1.5,
                kind,
                style,
                color: LayoutColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            };
            // Two zoom levels exercise the scale path. No panic = pass.
            crate::decor::paint_decoration(&mut scene, &deco, 1.0);
            crate::decor::paint_decoration(&mut scene, &deco, 2.5);
        }
    }
}

#[test]
fn test_paint_empty_layout_does_not_panic() {
    let layout = make_continuous_layout(vec![]);
    let mut scene = vello::Scene::new();
    let mut font_cache = FontDataCache::new();
    paint_layout(&mut scene, &layout, &mut font_cache, (0.0, 0.0), 1.0, None);
    // Reaching here without panic = pass.
}

#[test]
fn test_paint_filled_rect() {
    let layout = make_continuous_layout(vec![PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(10.0, 10.0, 100.0, 50.0),
        color: LayoutColor {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
    })]);
    let mut scene = vello::Scene::new();
    let mut font_cache = FontDataCache::new();
    paint_layout(&mut scene, &layout, &mut font_cache, (0.0, 0.0), 1.0, None);
    // No panic = pass.
}

#[test]
fn test_translate_item_glyph_run() {
    let mut item = PositionedItem::GlyphRun(PositionedGlyphRun {
        origin: LayoutPoint { x: 10.0, y: 20.0 },
        font_data: Arc::new(vec![]),
        font_index: 0,
        font_size: 12.0,
        glyphs: vec![],
        color: LayoutColor::BLACK,
        synthesis: GlyphSynthesis::default(),
        normalized_coords: Vec::new(),
        link_url: None,
    });
    translate_item(&mut item, 5.0, 3.0);
    if let PositionedItem::GlyphRun(r) = &item {
        assert_eq!(r.origin.x, 15.0);
        assert_eq!(r.origin.y, 23.0);
    } else {
        panic!("expected GlyphRun variant");
    }
}

#[test]
fn test_translate_item_filled_rect() {
    let mut item = PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(0.0, 0.0, 50.0, 50.0),
        color: LayoutColor::WHITE,
    });
    translate_item(&mut item, 10.0, 20.0);
    if let PositionedItem::FilledRect(r) = &item {
        assert_eq!(r.rect.x(), 10.0);
        assert_eq!(r.rect.y(), 20.0);
    } else {
        panic!("expected FilledRect variant");
    }
}

#[test]
fn test_paint_with_scale_factor() {
    let layout = make_continuous_layout(vec![PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(0.0, 0.0, 100.0, 100.0),
        color: LayoutColor::BLACK,
    })]);
    let mut scene = vello::Scene::new();
    let mut font_cache = FontDataCache::new();
    // 2× HiDPI scale.
    paint_layout(&mut scene, &layout, &mut font_cache, (0.0, 0.0), 2.0, None);
    // No panic = pass.
}

#[test]
fn test_paint_clipped_group_does_not_panic() {
    // Construct a ClippedGroup containing a FilledRect and verify paint_items
    // does not panic. Full visual correctness is verified manually.
    let inner_rect = PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(0.0, 0.0, 100.0, 20.0),
        color: LayoutColor {
            r: 0.0,
            g: 0.5,
            b: 1.0,
            a: 1.0,
        },
    });
    let layout = make_continuous_layout(vec![PositionedItem::ClippedGroup {
        clip_rect: LayoutRect::new(0.0, 0.0, 100.0, 15.0),
        items: vec![inner_rect],
    }]);
    let mut scene = vello::Scene::new();
    let mut font_cache = FontDataCache::new();
    paint_layout(&mut scene, &layout, &mut font_cache, (5.0, 10.0), 1.0, None);
    // No panic = pass.
}

#[test]
fn test_translate_item_clipped_group() {
    // Verify that translate_item shifts both clip_rect.origin and child items.
    let mut item = PositionedItem::ClippedGroup {
        clip_rect: LayoutRect::new(10.0, 20.0, 100.0, 50.0),
        items: vec![PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(0.0, 0.0, 50.0, 25.0),
            color: LayoutColor::WHITE,
        })],
    };
    translate_item(&mut item, 5.0, 3.0);
    if let PositionedItem::ClippedGroup { clip_rect, items } = &item {
        assert_eq!(clip_rect.x(), 15.0, "clip_rect x should be shifted");
        assert_eq!(clip_rect.y(), 23.0, "clip_rect y should be shifted");
        if let PositionedItem::FilledRect(r) = &items[0] {
            assert_eq!(r.rect.x(), 5.0, "child item x should be shifted");
            assert_eq!(r.rect.y(), 3.0, "child item y should be shifted");
        }
    } else {
        panic!("expected ClippedGroup");
    }
}

#[test]
fn test_paint_rotated_group_does_not_panic() {
    let inner_rect = PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(0.0, 0.0, 100.0, 20.0),
        color: LayoutColor {
            r: 0.0,
            g: 0.5,
            b: 1.0,
            a: 1.0,
        },
    });
    let layout = make_continuous_layout(vec![PositionedItem::RotatedGroup {
        origin: LayoutPoint { x: 10.0, y: 10.0 },
        degrees: 90.0,
        content_width: 100.0,
        content_height: 20.0,
        items: vec![inner_rect],
    }]);
    let mut scene = vello::Scene::new();
    let mut font_cache = FontDataCache::new();
    paint_layout(&mut scene, &layout, &mut font_cache, (5.0, 10.0), 1.0, None);
    // No panic = pass.
}

#[test]
fn test_translate_item_rotated_group() {
    let mut item = PositionedItem::RotatedGroup {
        origin: LayoutPoint { x: 10.0, y: 20.0 },
        degrees: 90.0,
        content_width: 100.0,
        content_height: 50.0,
        items: vec![PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(0.0, 0.0, 50.0, 25.0),
            color: LayoutColor::WHITE,
        })],
    };
    translate_item(&mut item, 5.0, 3.0);
    if let PositionedItem::RotatedGroup { origin, items, .. } = &item {
        assert_eq!(origin.x, 15.0, "origin x should be shifted");
        assert_eq!(origin.y, 23.0, "origin y should be shifted");
        if let PositionedItem::FilledRect(r) = &items[0] {
            assert_eq!(r.rect.x(), 0.0, "child item x should not be shifted");
            assert_eq!(r.rect.y(), 0.0, "child item y should not be shifted");
        }
    } else {
        panic!("expected RotatedGroup");
    }
}
