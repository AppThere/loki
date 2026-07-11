// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level scene painting functions.
//!
//! The entry point is [`paint_layout`], which dispatches to either
//! [`paint_paginated`] or [`paint_continuous`] depending on the layout kind.
//! These functions translate a [`loki_layout::DocumentLayout`] into Vello draw
//! commands appended to a [`vello::Scene`].

use vello::kurbo::Affine;
use vello::peniko::BlendMode;

use loki_layout::{
    ContinuousLayout, CursorRect, DocumentLayout, LayoutColor, LayoutPage, LayoutRect,
    PaginatedLayout, PositionedGlyphRun, PositionedItem, PositionedRect,
};

use crate::font_cache::FontDataCache;

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

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Paint a slice of [`PositionedItem`]s, translating each by `offset`.
///
/// Rather than pushing a Vello layer for the offset, we adjust item
/// coordinates directly to avoid layer overhead.
pub(crate) fn paint_items(
    scene: &mut vello::Scene,
    items: &[PositionedItem],
    font_cache: &mut FontDataCache,
    offset: (f32, f32),
    scale: f32,
) {
    for item in items {
        // Fast paths for the variants whose clone would allocate: a `GlyphRun`
        // carries a `Vec<GlyphEntry>` and the groups carry a child `Vec`, so the
        // old `item.clone()` copied the whole run / subtree just to shift its
        // origin. Paint these with an explicit `offset` instead. The cheap,
        // all-`Copy` leaf variants (rects, decorations, rules) still take the
        // clone-and-translate path below — their clone is a stack copy.
        match item {
            PositionedItem::GlyphRun(r) => {
                // Link visual hint (gap #11): paint a translucent blue underlay
                // behind runs that carry a hyperlink URL. Point→URL hit-testing
                // is available via loki-layout's `link_at` (feature 5.11).
                if r.link_url.is_some() {
                    paint_link_hint(scene, r, scale, offset);
                }
                crate::glyph::paint_glyph_run(scene, r, font_cache, scale, offset);
                continue;
            }
            PositionedItem::ClippedGroup { clip_rect, items } => {
                // ADR 004 open question 1: verified Vello 0.6 push_layer signature:
                //   fn push_layer(&mut self, blend: impl Into<BlendMode>, alpha: f32,
                //                 transform: Affine, clip: &impl Shape)
                // The clip rect is offset inline ((coord + offset) * scale); child
                // items inherit the same `offset` (no pre-translation / no clone).
                scene.push_layer(
                    BlendMode::default(),
                    1.0,
                    Affine::IDENTITY,
                    &vello::kurbo::Rect::new(
                        ((clip_rect.x() + offset.0) * scale) as f64,
                        ((clip_rect.y() + offset.1) * scale) as f64,
                        ((clip_rect.max_x() + offset.0) * scale) as f64,
                        ((clip_rect.max_y() + offset.1) * scale) as f64,
                    ),
                );
                paint_items(scene, items, font_cache, offset, scale);
                scene.pop_layer();
                continue;
            }
            PositionedItem::RotatedGroup {
                origin,
                degrees,
                content_width,
                content_height,
                items,
            } => {
                // Origin is offset inline; the rotated children are painted into a
                // temporary scene with no offset and appended under the rotation.
                let ox = origin.x + offset.0;
                let oy = origin.y + offset.1;
                let cx_local = content_width / 2.0;
                let cy_local = content_height / 2.0;

                let (cx_physical, cy_physical) = match *degrees as i32 {
                    90 | 270 => (ox + cy_local, oy + cx_local),
                    _ => (ox + cx_local, oy + cy_local),
                };

                let angle = (*degrees as f64).to_radians();

                let transform =
                    Affine::translate(((cx_physical * scale) as f64, (cy_physical * scale) as f64))
                        * Affine::rotate(angle)
                        * Affine::translate((
                            -(cx_local * scale) as f64,
                            -(cy_local * scale) as f64,
                        ));

                let local_clip = vello::kurbo::Rect::new(
                    0.0,
                    0.0,
                    (content_width * scale) as f64,
                    (content_height * scale) as f64,
                );

                // COMPAT(vello-0.6): push_layer's `transform` only applies to
                // the clip shape, NOT to content drawn inside the layer (see
                // Vello 0.6 Scene::push_layer docs). To rotate actual content
                // we draw into a temporary scene and append it with the
                // rotation transform via Scene::append, which DOES transform
                // all drawing operations.
                scene.push_layer(BlendMode::default(), 1.0, transform, &local_clip);
                let mut rotated_scene = vello::Scene::new();
                paint_items(&mut rotated_scene, items, font_cache, (0.0, 0.0), scale);
                scene.append(&rotated_scene, Some(transform));
                scene.pop_layer();
                continue;
            }
            // Cheap leaf variants fall through to the clone-and-translate path.
            _ => {}
        }

        // Leaf variants are small, all-`Copy` structs (or a rare image): cloning
        // one is a stack copy, so translate a clone in place and paint it.
        let mut item = item.clone();
        translate_item(&mut item, offset.0, offset.1);
        match &item {
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
                    color: LayoutColor {
                        r: 0.7,
                        g: 0.7,
                        b: 0.7,
                        a: 1.0,
                    },
                };
                crate::rect::paint_filled_rect(scene, &rule, scale);
            }
            _ => {
                // GlyphRun / groups handled above; `PositionedItem` is
                // `#[non_exhaustive]`, so ignore any other variant.
            }
        }
    }
}

/// Paint a translucent blue underlay rect behind a link glyph run (gap #11).
///
/// The hint uses the run's ascent and descent metrics to cover the text extent.
/// `PositionedGlyphRun` does not carry font metrics directly; a fixed-height
/// estimate based on font size is used (ascent ≈ 0.8 × font_size, descent ≈
/// 0.2 × font_size). This is approximate but sufficient for the visual hint.
fn paint_link_hint(
    scene: &mut vello::Scene,
    r: &PositionedGlyphRun,
    scale: f32,
    offset: (f32, f32),
) {
    let ascent = r.font_size * 0.8;
    let descent = r.font_size * 0.2;
    // Sum advance of all glyphs for the run width.
    let width: f32 = r.glyphs.iter().map(|g| g.advance).sum();
    if width <= 0.0 {
        return;
    }
    let hint = PositionedRect {
        rect: LayoutRect::new(
            r.origin.x + offset.0,
            r.origin.y - ascent + offset.1,
            width,
            ascent + descent,
        ),
        color: LayoutColor {
            r: 0.0,
            g: 0.4,
            b: 1.0,
            a: 0.15,
        },
    };
    crate::rect::paint_filled_rect(scene, &hint, scale);
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
        PositionedItem::ClippedGroup { clip_rect, items } => {
            clip_rect.origin.x += dx;
            clip_rect.origin.y += dy;
            for item in items {
                translate_item(item, dx, dy);
            }
        }
        PositionedItem::RotatedGroup { origin, .. } => {
            origin.x += dx;
            origin.y += dy;
        }
        _ => {
            // `PositionedItem` is `#[non_exhaustive]`; ignore unknown variants.
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
