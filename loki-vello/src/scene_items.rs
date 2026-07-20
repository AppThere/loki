// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Positioned-item painting (split from `scene.rs` for the 300-line ceiling):
//! `paint_items` walks a `[PositionedItem]` slice and dispatches each variant
//! to the per-kind painters (`crate::glyph` / `crate::rect` / `crate::image` /
//! `crate::decor`), recursing into clipped/rotated groups; plus the hyperlink
//! underlay hint and the leaf-item translate helper. `paint_items` is
//! re-exported from `scene.rs` (also used by the float-band painter).

use vello::kurbo::Affine;
use vello::peniko::BlendMode;

use loki_layout::{LayoutColor, LayoutRect, PositionedGlyphRun, PositionedItem, PositionedRect};

use crate::font_cache::FontDataCache;

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
            PositionedItem::HatchRect(h) => {
                crate::rect::paint_hatch(scene, h, scale);
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
pub(super) fn translate_item(item: &mut PositionedItem, dx: f32, dy: f32) {
    match item {
        PositionedItem::GlyphRun(r) => {
            r.origin.x += dx;
            r.origin.y += dy;
        }
        PositionedItem::FilledRect(r) => {
            r.rect.origin.x += dx;
            r.rect.origin.y += dy;
        }
        PositionedItem::HatchRect(h) => {
            h.rect.origin.x += dx;
            h.rect.origin.y += dy;
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
