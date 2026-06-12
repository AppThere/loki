// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level scene painting functions.
//!
//! The entry point is [`paint_layout`], which dispatches to either
//! [`paint_paginated`] or [`paint_continuous`] depending on the layout kind.
//! These functions translate a [`loki_layout::DocumentLayout`] into Vello draw
//! commands appended to a [`vello::Scene`].

mod continuous;
mod cursor;
mod items;
mod paginated;
mod types;

// ── Public re-exports ─────────────────────────────────────────────────────────

pub use continuous::paint_continuous;
pub use cursor::paint_cursor;
pub use paginated::{paint_paginated, paint_single_page};
pub use types::{CursorPaint, SelectionHandle, SelectionHandleKind, SelectionRect};

// ── Public API ────────────────────────────────────────────────────────────────

use loki_layout::DocumentLayout;

use crate::font_cache::FontDataCache;

/// Paint a complete [`DocumentLayout`] into a Vello scene.
///
/// Draw commands are *appended* to `scene`. The caller is responsible for
/// calling [`vello::Scene::reset`] before this call if the scene needs to be
/// cleared.
///
/// # Parameters
///
/// * `scene` – target Vello scene.
/// * `layout` – document layout produced by `loki-layout`.
/// * `font_cache` – reusable font-data cache; share across frames to avoid
///   redundant allocations.
/// * `offset` – `(x, y)` translation in layout points applied to the entire
///   document. Useful for placing the document canvas inside a larger UI.
/// * `scale` – display scale factor (`1.0` for 1× displays, `2.0` for HiDPI).
/// * `page_index` – when `Some(n)`, render only page `n` of a paginated layout
///   at the given `offset`; when `None`, render all pages stacked vertically.
///   Ignored for continuous layouts (all content is always painted).
///
/// Cursor and selection paint data are not supported through this entry point;
/// call [`paint_single_page`] directly when cursor rendering is needed.
///
/// # TODO(partial-render)
///
/// `page_index` is the first step toward viewport clipping: once per-page
/// canvases are in place, the scroll viewport can be compared against page
/// positions to skip rendering pages entirely outside the visible area.
pub fn paint_layout(
    scene: &mut vello::Scene,
    layout: &DocumentLayout,
    font_cache: &mut FontDataCache,
    offset: (f32, f32),
    scale: f32,
    page_index: Option<usize>,
) {
    match layout {
        DocumentLayout::Paginated(pl) => {
            if let Some(idx) = page_index {
                paint_single_page(scene, pl, font_cache, offset, scale, idx, None);
            } else {
                paint_paginated(scene, pl, font_cache, offset, scale);
            }
        }
        DocumentLayout::Continuous(cl) => paint_continuous(scene, cl, font_cache, offset, scale),
        // `DocumentLayout` is `#[non_exhaustive]`; silently ignore future variants.
        _ => {}
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loki_layout::{
        ContinuousLayout, DocumentLayout, GlyphSynthesis, LayoutColor, LayoutPoint, LayoutRect,
        PositionedGlyphRun, PositionedItem, PositionedRect,
    };

    use super::*;
    use crate::font_cache::FontDataCache;
    use items::translate_item;

    fn make_continuous_layout(items: Vec<PositionedItem>) -> DocumentLayout {
        DocumentLayout::Continuous(ContinuousLayout {
            content_width: 400.0,
            total_height: 100.0,
            items,
        })
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
}
