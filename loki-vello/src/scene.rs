// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Top-level scene painting functions.
//!
//! The entry point is [`paint_layout`], which dispatches to either
//! [`paint_paginated`] or [`paint_continuous`] depending on the layout kind.
//! These functions translate a [`loki_layout::DocumentLayout`] into Vello draw
//! commands appended to a [`vello::Scene`].

use loki_layout::{
    ContinuousLayout, DocumentLayout, LayoutColor, LayoutRect, PaginatedLayout, PositionedItem,
    PositionedRect,
};

use crate::font_cache::FontDataCache;

// ── Visual constants for paginated layout ────────────────────────────────────

const PAGE_GAP_PT: f32 = 16.0;
const PAGE_SHADOW_COLOR: LayoutColor = LayoutColor { r: 0.6, g: 0.6, b: 0.6, a: 0.4 };
const PAGE_BG_COLOR: LayoutColor = LayoutColor { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

// ── Public API ────────────────────────────────────────────────────────────────

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
                paint_single_page(scene, pl, font_cache, offset, scale, idx);
            } else {
                paint_paginated(scene, pl, font_cache, offset, scale);
            }
        }
        DocumentLayout::Continuous(cl) => paint_continuous(scene, cl, font_cache, offset, scale),
        // `DocumentLayout` is `#[non_exhaustive]`; silently ignore future variants.
        _ => {}
    }
}

/// Paint a single page from a paginated layout at the given `offset`.
///
/// Content items in the layout are already page-local (translated by
/// [`loki_layout::layout_document`]). This function renders the page
/// background and shadow as if the page top-left is at `offset`.
///
/// Out-of-range `page_index` values are silently ignored.
pub fn paint_single_page(
    scene: &mut vello::Scene,
    layout: &PaginatedLayout,
    font_cache: &mut FontDataCache,
    offset: (f32, f32),
    scale: f32,
    page_index: usize,
) {
    let Some(page) = layout.pages.get(page_index) else {
        return;
    };

    let page_width = layout.page_size.width;
    let page_height = layout.page_size.height;

    // Drop shadow slightly behind the page.
    let shadow = PositionedRect {
        rect: LayoutRect::new(offset.0 + 4.0, offset.1 + 4.0, page_width, page_height),
        color: PAGE_SHADOW_COLOR,
    };
    crate::rect::paint_filled_rect(scene, &shadow, scale);

    // White page background.
    let page_bg = PositionedRect {
        rect: LayoutRect::new(offset.0, offset.1, page_width, page_height),
        color: PAGE_BG_COLOR,
    };
    crate::rect::paint_filled_rect(scene, &page_bg, scale);

    let page_offset = (offset.0, offset.1);
    paint_items(scene, &page.content_items, font_cache, page_offset, scale);
    paint_items(scene, &page.header_items, font_cache, page_offset, scale);
    paint_items(scene, &page.footer_items, font_cache, page_offset, scale);
}

/// Paint a paginated layout.
///
/// Pages are arranged vertically with [`PAGE_GAP_PT`] points of space between
/// them, as in a typical word-processor print preview. Each page gets a white
/// background and a subtle translucent drop shadow.
pub fn paint_paginated(
    scene: &mut vello::Scene,
    layout: &PaginatedLayout,
    font_cache: &mut FontDataCache,
    offset: (f32, f32),
    scale: f32,
) {
    let mut y_cursor = offset.1;

    for page in &layout.pages {
        let page_width = layout.page_size.width;
        let page_height = layout.page_size.height;

        // Draw the drop shadow behind the page (slightly offset, semi-transparent).
        let shadow = PositionedRect {
            rect: LayoutRect::new(offset.0 + 4.0, y_cursor + 4.0, page_width, page_height),
            color: PAGE_SHADOW_COLOR,
        };
        crate::rect::paint_filled_rect(scene, &shadow, scale);

        // Draw the white page background on top of the shadow.
        let page_bg = PositionedRect {
            rect: LayoutRect::new(offset.0, y_cursor, page_width, page_height),
            color: PAGE_BG_COLOR,
        };
        crate::rect::paint_filled_rect(scene, &page_bg, scale);

        // Paint all three item lists using the page's top-left as the origin.
        let page_offset = (offset.0, y_cursor);
        paint_items(scene, &page.content_items, font_cache, page_offset, scale);
        paint_items(scene, &page.header_items, font_cache, page_offset, scale);
        paint_items(scene, &page.footer_items, font_cache, page_offset, scale);

        y_cursor += page_height + PAGE_GAP_PT;
    }
}

/// Paint a continuous (pageless / reflow) layout onto a single canvas.
pub fn paint_continuous(
    scene: &mut vello::Scene,
    layout: &ContinuousLayout,
    font_cache: &mut FontDataCache,
    offset: (f32, f32),
    scale: f32,
) {
    paint_items(scene, &layout.items, font_cache, offset, scale);
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Paint a slice of [`PositionedItem`]s, translating each by `offset`.
///
/// Rather than pushing a Vello layer for the offset, we adjust item
/// coordinates directly to avoid layer overhead.
fn paint_items(
    scene: &mut vello::Scene,
    items: &[PositionedItem],
    font_cache: &mut FontDataCache,
    offset: (f32, f32),
    scale: f32,
) {
    for item in items {
        let mut item = item.clone();
        translate_item(&mut item, offset.0, offset.1);
        match &item {
            PositionedItem::GlyphRun(r) => {
                crate::glyph::paint_glyph_run(scene, r, font_cache, scale);
            }
            PositionedItem::FilledRect(r) => {
                crate::rect::paint_filled_rect(scene, r, scale);
            }
            PositionedItem::BorderRect(r) => {
                crate::rect::paint_border_rect(scene, r, scale);
            }
            PositionedItem::Image(img) => {
                // Ignore image errors during layout rendering; a failed image
                // leaves the scene unchanged.
                let _ = crate::image::paint_image(scene, img, scale);
            }
            PositionedItem::Decoration(d) => {
                crate::decor::paint_decoration(scene, d, scale);
            }
            PositionedItem::HorizontalRule(r) => {
                // Render as a thin grey filled rectangle.
                let rule = PositionedRect {
                    rect: r.rect,
                    color: LayoutColor { r: 0.7, g: 0.7, b: 0.7, a: 1.0 },
                };
                crate::rect::paint_filled_rect(scene, &rule, scale);
            }
            _ => {
                // `PositionedItem` is `#[non_exhaustive]`; ignore unknown variants.
            }
        }
    }
}

/// Apply an `(dx, dy)` translation to a [`PositionedItem`] in place.
///
/// This adjusts coordinates at the leaf level instead of using a Vello
/// transform layer, which avoids per-item layer overhead.
fn translate_item(item: &mut PositionedItem, dx: f32, dy: f32) {
    match item {
        PositionedItem::GlyphRun(r) => {
            r.origin.x += dx;
            r.origin.y += dy;
        }
        PositionedItem::FilledRect(r) => {
            r.rect.origin.x += dx;
            r.rect.origin.y += dy;
        }
        PositionedItem::BorderRect(r) => {
            r.rect.origin.x += dx;
            r.rect.origin.y += dy;
        }
        PositionedItem::Image(r) => {
            r.rect.origin.x += dx;
            r.rect.origin.y += dy;
        }
        PositionedItem::Decoration(d) => {
            d.x += dx;
            d.y += dy;
        }
        PositionedItem::HorizontalRule(r) => {
            r.rect.origin.x += dx;
            r.rect.origin.y += dy;
        }
        _ => {
            // `PositionedItem` is `#[non_exhaustive]`; no translation for unknown variants.
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use loki_layout::{
        GlyphSynthesis, LayoutColor, LayoutPoint, LayoutRect, PositionedGlyphRun, PositionedItem,
        PositionedRect,
    };

    use super::*;
    use crate::font_cache::FontDataCache;

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
            color: LayoutColor { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
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
}
