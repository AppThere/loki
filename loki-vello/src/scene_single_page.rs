// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Single-page painting (split from `scene.rs` for the 300-line ceiling):
//! `paint_single_page` draws one paginated page — L-shaped drop shadow, white
//! background, content/header/footer/comment items, and the optional
//! rotation-aware cursor + selection overlay. Re-exported from `scene.rs`;
//! reaches the page-chrome consts/helper and `paint_items` via `super::`.

use loki_layout::{CursorRect, LayoutRect, PaginatedLayout, PositionedRect};

use super::{
    CursorPaint, PAGE_BG_COLOR, PAGE_SHADOW_COLOR, PAGE_SHADOW_OFFSET, page_chrome_size,
    paint_items,
};
use crate::font_cache::FontDataCache;

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

    // Per-page size (see `page_chrome_size`): never the document-level default.
    let (page_width, page_height) = page_chrome_size(page);

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
    // Comment-panel items are page-local but extend into the gutter to the
    // right of the page.
    paint_items(scene, &page.comment_items, font_cache, page_origin, scale);

    // Cursor and selection highlights — painted after content so they appear
    // on top of glyphs.
    if let Some(cp) = cursor_paint {
        // Multi-paragraph selection spans: each span is transformed by its own
        // paragraph's per-page origin (and rotation), so selections spanning
        // paragraphs or page-split fragments highlight correctly.
        for span in &cp.selection_spans {
            let span_para = page.editing_data.as_ref().and_then(|ed| {
                ed.paragraphs
                    .iter()
                    .find(|p| p.block_index == span.paragraph_index)
            });
            if span_para.is_none() {
                continue;
            }
            let t = crate::scene_cursor::cursor_paint_transform(span_para, content_origin, scale);
            crate::scene_cursor::paint_cursor_transformed(
                scene,
                &CursorRect {
                    x: 0.0,
                    y: 0.0,
                    height: 0.0,
                },
                &span.rects,
                &span.handles,
                t,
            );
        }

        // The cursor rect and selection rects are in paragraph-local coordinates.
        // Find the paragraph fragment on this page that matches the global
        // paragraph_index, and use its origin.
        let para_data = page.editing_data.as_ref().and_then(|ed| {
            ed.paragraphs
                .iter()
                .find(|p| p.block_index == cp.paragraph_index)
        });

        // Rotation-aware: a rotated table cell's caret/selection tilt with the
        // text (the transform composes the cell's rotation affine).
        let transform =
            crate::scene_cursor::cursor_paint_transform(para_data, content_origin, scale);

        if let Some(cr) = cp.cursor_rect.as_ref() {
            crate::scene_cursor::paint_cursor_transformed(
                scene,
                cr,
                &cp.selection_rects,
                &cp.selection_handles,
                transform,
            );
        } else if !cp.selection_rects.is_empty() || !cp.selection_handles.is_empty() {
            crate::scene_cursor::paint_cursor_transformed(
                scene,
                // Dummy zero-size rect when only selection highlights / handles are needed.
                &CursorRect {
                    x: 0.0,
                    y: 0.0,
                    height: 0.0,
                },
                &cp.selection_rects,
                &cp.selection_handles,
                transform,
            );
        }
    }
}
