// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Band painting for continuous (reflow) layouts.
//!
//! A continuous layout is one tall canvas, but GPU textures cannot hold a whole
//! document, so the renderer slices it into fixed-height horizontal bands and
//! paints each band into its own texture tile. [`paint_continuous_band`] paints
//! the items overlapping one band, translated so the band's top edge maps to
//! `y = 0` in the texture.
//!
//! Items that span a band boundary are painted into **both** adjacent bands.
//! Each copy is clipped by its texture edge, and because both copies use
//! identical layout coordinates the visible halves abut pixel-perfectly when
//! the tiles are stacked with zero gap — the same effect as scrolling a
//! viewport over the canvas.

use loki_layout::{ContinuousLayout, PositionedItem};

use crate::FontDataCache;
use crate::scene::paint_items;

/// Extra margin (in points) added around each item's estimated vertical extent
/// when deciding band membership.  Over-inclusion only costs a duplicate paint
/// that the texture edge clips away; under-inclusion would drop content.
const EXTENT_SLOP_PT: f32 = 4.0;

/// Conservative vertical extent `(top, bottom)` of an item in layout points.
///
/// Rect-backed items report exact bounds.  Glyph runs only carry their top-left
/// origin and font size, so their extent is over-estimated (one em above the
/// origin, three below) — safe for band-membership tests, not for tight
/// geometry.
fn y_extent(item: &PositionedItem) -> (f32, f32) {
    match item {
        PositionedItem::GlyphRun(r) => (r.origin.y - r.font_size, r.origin.y + r.font_size * 3.0),
        PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => {
            (r.rect.origin.y, r.rect.origin.y + r.rect.size.height)
        }
        PositionedItem::BorderRect(r) => (r.rect.origin.y, r.rect.origin.y + r.rect.size.height),
        PositionedItem::Image(img) => (img.rect.origin.y, img.rect.origin.y + img.rect.size.height),
        PositionedItem::Decoration(d) => (d.y - d.thickness, d.y + d.thickness * 2.0),
        // Children are masked by the clip rect, so it bounds the visible area.
        PositionedItem::ClippedGroup { clip_rect, .. } => (
            clip_rect.origin.y,
            clip_rect.origin.y + clip_rect.size.height,
        ),
        // A rotated cell fits inside a square of its larger dimension.
        PositionedItem::RotatedGroup {
            origin,
            content_width,
            content_height,
            ..
        } => {
            let max_dim = content_width.max(*content_height);
            (origin.y, origin.y + max_dim)
        }
        // `PositionedItem` is `#[non_exhaustive]`; treat unknown variants as
        // unbounded so they are painted into every band rather than dropped.
        _ => (f32::MIN, f32::MAX),
    }
}

/// Right-edge x of an item in layout points (conservative over-estimate).
///
/// Used to size reflow tiles to the widest content so wide elements (e.g.
/// fixed-width tables) can be reached by horizontal scrolling instead of being
/// clipped.
fn x_extent(item: &PositionedItem) -> f32 {
    match item {
        PositionedItem::GlyphRun(r) => {
            let run_w = r
                .glyphs
                .iter()
                .map(|g| g.x + g.advance)
                .fold(0.0_f32, f32::max);
            r.origin.x + run_w
        }
        PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => {
            r.rect.origin.x + r.rect.size.width
        }
        PositionedItem::BorderRect(r) => r.rect.origin.x + r.rect.size.width,
        PositionedItem::Image(img) => img.rect.origin.x + img.rect.size.width,
        PositionedItem::Decoration(d) => d.x + d.width,
        // Children are masked to the clip rect, so it bounds the visible width.
        PositionedItem::ClippedGroup { clip_rect, .. } => clip_rect.origin.x + clip_rect.size.width,
        PositionedItem::RotatedGroup {
            origin,
            content_width,
            content_height,
            ..
        } => origin.x + content_width.max(*content_height),
        // `PositionedItem` is `#[non_exhaustive]`; unknown variants contribute
        // nothing to the width estimate.
        _ => 0.0,
    }
}

/// Maximum right-edge x over all items of `layout`, in layout points.
pub fn content_max_x(layout: &ContinuousLayout) -> f32 {
    layout.items.iter().map(x_extent).fold(0.0_f32, f32::max)
}

/// Paint the items of `layout` that overlap the vertical band
/// `[band_top, band_top + band_height)` (layout points), translated so the
/// band top maps to `y = 0` and shifted right by `x_offset` points.
///
/// Used by the reflow render path: each texture tile is one band.  `x_offset`
/// provides the horizontal reading-margin inset (the layout itself is computed
/// at `tile width − 2 × inset`).
pub fn paint_continuous_band(
    scene: &mut vello::Scene,
    layout: &ContinuousLayout,
    font_cache: &mut FontDataCache,
    x_offset: f32,
    scale: f32,
    band_top: f32,
    band_height: f32,
) {
    let band_bottom = band_top + band_height;
    for item in &layout.items {
        let (top, bottom) = y_extent(item);
        if bottom + EXTENT_SLOP_PT < band_top || top - EXTENT_SLOP_PT > band_bottom {
            continue;
        }
        paint_items(
            scene,
            std::slice::from_ref(item),
            font_cache,
            (x_offset, -band_top),
            scale,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_layout::{LayoutColor, LayoutRect, PositionedRect};

    fn rect(x: f32, w: f32) -> PositionedItem {
        PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(x, 0.0, w, 10.0),
            color: LayoutColor::BLACK,
        })
    }

    #[test]
    fn content_max_x_uses_widest_right_edge() {
        let layout = ContinuousLayout {
            content_width: 300.0,
            total_height: 100.0,
            items: vec![rect(0.0, 200.0), rect(100.0, 500.0), rect(50.0, 10.0)],
        };
        // Widest right edge is 100 + 500 = 600.
        assert_eq!(content_max_x(&layout), 600.0);
    }

    #[test]
    fn content_max_x_empty_is_zero() {
        let layout = ContinuousLayout {
            content_width: 300.0,
            total_height: 0.0,
            items: vec![],
        };
        assert_eq!(content_max_x(&layout), 0.0);
    }
}
