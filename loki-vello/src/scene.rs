// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level scene painting functions.
//!
//! The entry point is [`paint_layout`], which dispatches to either
//! [`paint_paginated`] or [`paint_continuous`] depending on the layout kind.
//! These functions translate a [`loki_layout::DocumentLayout`] into Vello draw
//! commands appended to a [`vello::Scene`].

use loki_layout::{
    ContinuousLayout, CursorRect, DocumentLayout, LayoutColor, LayoutPage, LayoutRect,
    PaginatedLayout, PositionedRect,
};

use crate::font_cache::FontDataCache;

#[path = "scene_items.rs"]
mod items;
pub(crate) use items::paint_items;

// ── Cursor and selection rendering types ─────────────────────────────────────
// (The painting functions live in `scene_cursor.rs`.)

/// Whether a selection handle is at the anchor (start) or focus (end) of the
/// selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionHandleKind {
    /// Anchor handle — shown at the start of the selection.
    Anchor,
    /// Focus handle — shown at the end of the selection.
    Focus,
}

/// A teardrop-shaped selection handle rendered at the edge of a mobile selection.
///
/// Handles are only shown on iOS and Android (controlled by `#[cfg(target_os)]`
/// in `editor.rs`). On desktop the cursor and selection highlights are
/// sufficient — drag handles would look out of place.
#[derive(Debug, Clone)]
pub struct SelectionHandle {
    /// X position of the handle tip in page-content-area coordinates (points).
    pub tip_x: f32,
    /// Y position of the handle tip in page-content-area coordinates (points).
    pub tip_y: f32,
    /// Whether this is the anchor (start) or focus (end) handle.
    pub kind: SelectionHandleKind,
}

/// A highlight rectangle for a selection range, in paragraph-local coordinates
/// (points).
#[derive(Debug, Clone, Copy)]
pub struct SelectionRect {
    /// X position of the rectangle's left edge in paragraph-local coordinates.
    pub x: f32,
    /// Y position of the rectangle's top edge in paragraph-local coordinates.
    pub y: f32,
    /// Width of the rectangle in points.
    pub width: f32,
    /// Height of the rectangle in points.
    pub height: f32,
}

/// Cursor and selection highlight data for a single paragraph on one page.
///
/// All rects are in paragraph-local coordinates (points, origin at the
/// paragraph's `(0, 0)` top-left). The painter applies the paragraph origin
/// and page content-area offset at render time.
#[derive(Debug, Clone)]
pub struct CursorPaint {
    /// Visual cursor rect, or `None` when the cursor has no position in this
    /// paragraph.
    pub cursor_rect: Option<CursorRect>,
    /// Zero or more selection highlight rects.  Empty when no range selection
    /// is active.
    pub selection_rects: Vec<SelectionRect>,
    /// Selection handles for mobile (iOS/Android) long-press word selection.
    ///
    /// Populated only when a range selection is active on a touch device.
    /// Empty on desktop — handles are guarded by `#[cfg(target_os)]` in the
    /// caller.
    pub selection_handles: Vec<SelectionHandle>,
    /// Global index of the paragraph block that this data belongs to.
    /// Used by the painter to look up the paragraph's page-local origin.
    pub paragraph_index: usize,
}

// ── Visual constants for paginated layout ────────────────────────────────────

const PAGE_GAP_PT: f32 = 16.0;
// TODO(shadow): replace with Vello blur filter once rendering is verified stable.
// rgba8(0,0,0,40) — darker than before and placed only on right/bottom edges to
// avoid the gray vertical bar caused by the old shadow rect extending 4 px past
// the page background's right edge.
const PAGE_SHADOW_COLOR: LayoutColor = LayoutColor {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 40.0 / 255.0,
};
const PAGE_SHADOW_OFFSET: f32 = 3.0;
const PAGE_BG_COLOR: LayoutColor = LayoutColor {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// Physical size used to paint a page's white background and drop shadow.
///
/// Always the page's *own* size — never the document-level `layout.page_size`
/// default. The render tiles are textured at the per-page size
/// ([`loki_renderer`'s `page_size_pts`]), so a section with a different size or
/// orientation (A4, or landscape US Letter inside a portrait document) must
/// have its chrome painted at that page's dimensions; using the document
/// default leaves a mis-sized white rect and the canvas shows through as a gray
/// streak.
fn page_chrome_size(page: &LayoutPage) -> (f32, f32) {
    (page.page_size.width, page.page_size.height)
}

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
        // Per-page size (see `page_chrome_size`): a mixed-size document paints
        // each page's background/shadow at that page's own dimensions, else
        // differently-sized pages leave a gray streak.
        let (page_width, page_height) = page_chrome_size(page);

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
        paint_items(scene, &page.comment_items, font_cache, page_origin, scale);

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

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
