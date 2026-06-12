// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paginated layout painting: `paint_paginated` and `paint_single_page`.

use loki_layout::{CursorRect, LayoutColor, LayoutRect, PaginatedLayout, PositionedRect};

use crate::font_cache::FontDataCache;

use super::cursor::paint_cursor;
use super::items::paint_items;
use super::types::CursorPaint;

// ── Visual constants for paginated layout ────────────────────────────────────

pub(super) const PAGE_GAP_PT: f32 = 16.0;
// TODO(shadow): replace with Vello blur filter once rendering is verified stable.
// rgba8(0,0,0,40) — darker than before and placed only on right/bottom edges to
// avoid the gray vertical bar caused by the old shadow rect extending 4 px past
// the page background's right edge.
pub(super) const PAGE_SHADOW_COLOR: LayoutColor = LayoutColor {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 40.0 / 255.0,
};
pub(super) const PAGE_SHADOW_OFFSET: f32 = 3.0;
pub(super) const PAGE_BG_COLOR: LayoutColor = LayoutColor {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// Paint a single page from a paginated layout at the given `offset`.
///
/// Content items are in content-area-local coordinates (origin `(0, 0)` at
/// the content-area top-left). This function applies `page.margins` when
/// translating items onto the full page canvas, so the caller only needs to
/// supply the page top-left as `offset`.
///
/// `cursor_paint` carries optional cursor and selection highlight data for
/// the editing layer. Pass `None` in read-only mode — no cursor is drawn.
///
/// Out-of-range `page_index` values are silently ignored.
pub fn paint_single_page(
    scene: &mut vello::Scene,
    layout: &PaginatedLayout,
    font_cache: &mut FontDataCache,
    offset: (f32, f32),
    scale: f32,
    page_index: usize,
    cursor_paint: Option<&CursorPaint>,
) {
    let Some(page) = layout.pages.get(page_index) else {
        return;
    };

    let page_width = layout.page_size.width;
    let page_height = layout.page_size.height;

    // L-shaped drop shadow: right strip and bottom strip, each PAGE_SHADOW_OFFSET
    // wide, placed flush with the page bg edges. Never extends past max_x of the
    // page bg, eliminating the gray vertical bar visible on wide canvases.
    crate::rect::paint_filled_rect(
        scene,
        &PositionedRect {
            rect: LayoutRect::new(
                offset.0 + page_width,
                offset.1 + PAGE_SHADOW_OFFSET,
                PAGE_SHADOW_OFFSET,
                page_height,
            ),
            color: PAGE_SHADOW_COLOR,
        },
        scale,
    );
    crate::rect::paint_filled_rect(
        scene,
        &PositionedRect {
            rect: LayoutRect::new(
                offset.0 + PAGE_SHADOW_OFFSET,
                offset.1 + page_height,
                page_width,
                PAGE_SHADOW_OFFSET,
            ),
            color: PAGE_SHADOW_COLOR,
        },
        scale,
    );

    // White page background (painted after shadow so it covers the top-left corner).
    let page_bg = PositionedRect {
        rect: LayoutRect::new(offset.0, offset.1, page_width, page_height),
        color: PAGE_BG_COLOR,
    };
    crate::rect::paint_filled_rect(scene, &page_bg, scale);

    // content_items are in content-area-local coordinates; apply margins to
    // position within the full page.  header/footer items use page-local
    // coordinates, so they receive the raw page origin.
    let page_origin = (offset.0, offset.1);
    let content_origin = (offset.0 + page.margins.left, offset.1 + page.margins.top);
    paint_items(
        scene,
        &page.content_items,
        font_cache,
        content_origin,
        scale,
    );
    paint_items(scene, &page.header_items, font_cache, page_origin, scale);
    paint_items(scene, &page.footer_items, font_cache, page_origin, scale);

    // Cursor and selection highlights — painted after content so they appear
    // on top of glyphs.
    if let Some(cp) = cursor_paint {
        // The cursor rect and selection rects are in paragraph-local coordinates.
        // Find the paragraph fragment on this page that matches the global
        // paragraph_index, and use its origin.
        let para_data = page.editing_data.as_ref().and_then(|ed| {
            ed.paragraphs
                .iter()
                .find(|p| p.block_index == cp.paragraph_index)
        });

        let para_origin = para_data.map(|p| p.origin).unwrap_or((0.0, 0.0));

        let para_offset = (
            content_origin.0 + para_origin.0,
            content_origin.1 + para_origin.1,
        );

        if let Some(cr) = cp.cursor_rect.as_ref() {
            paint_cursor(
                scene,
                cr,
                &cp.selection_rects,
                &cp.selection_handles,
                para_offset,
                scale,
            );
        } else if !cp.selection_rects.is_empty() || !cp.selection_handles.is_empty() {
            paint_cursor(
                scene,
                // Dummy zero-size rect when only selection highlights / handles are needed.
                &CursorRect {
                    x: 0.0,
                    y: 0.0,
                    height: 0.0,
                },
                &cp.selection_rects,
                &cp.selection_handles,
                para_offset,
                scale,
            );
        }
    }
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

        // L-shaped drop shadow (right strip + bottom strip).
        crate::rect::paint_filled_rect(
            scene,
            &PositionedRect {
                rect: LayoutRect::new(
                    offset.0 + page_width,
                    y_cursor + PAGE_SHADOW_OFFSET,
                    PAGE_SHADOW_OFFSET,
                    page_height,
                ),
                color: PAGE_SHADOW_COLOR,
            },
            scale,
        );
        crate::rect::paint_filled_rect(
            scene,
            &PositionedRect {
                rect: LayoutRect::new(
                    offset.0 + PAGE_SHADOW_OFFSET,
                    y_cursor + page_height,
                    page_width,
                    PAGE_SHADOW_OFFSET,
                ),
                color: PAGE_SHADOW_COLOR,
            },
            scale,
        );

        // White page background (painted after shadow).
        let page_bg = PositionedRect {
            rect: LayoutRect::new(offset.0, y_cursor, page_width, page_height),
            color: PAGE_BG_COLOR,
        };
        crate::rect::paint_filled_rect(scene, &page_bg, scale);

        // content_items are content-area-local; apply per-page margins.
        // header/footer items use page-local coordinates.
        let page_origin = (offset.0, y_cursor);
        let content_origin = (offset.0 + page.margins.left, y_cursor + page.margins.top);
        paint_items(
            scene,
            &page.content_items,
            font_cache,
            content_origin,
            scale,
        );
        paint_items(scene, &page.header_items, font_cache, page_origin, scale);
        paint_items(scene, &page.footer_items, font_cache, page_origin, scale);

        y_cursor += page_height + PAGE_GAP_PT;
    }
}
