// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Glyph run rendering.
//!
//! Translates a [`loki_layout::PositionedGlyphRun`] into a Vello
//! `draw_glyphs` call.  Font data caching is handled by [`FontDataCache`].

use loki_layout::PositionedGlyphRun;

use crate::font_cache::FontDataCache;

/// Paint a single [`PositionedGlyphRun`] into a Vello scene.
///
/// `scale` is the display scale factor (1.0 for 1× displays, 2.0 for HiDPI).
/// Glyph runs with empty `font_data` or empty `glyphs` are silently skipped.
pub fn paint_glyph_run(
    scene: &mut vello::Scene,
    run: &PositionedGlyphRun,
    font_cache: &mut FontDataCache,
    scale: f32,
) {
    if run.glyphs.is_empty() {
        return;
    }
    if run.font_data.is_empty() {
        return;
    }

    let font = font_cache.get_or_insert(&run.font_data, run.font_index);

    // Translate to the run's baseline origin in scaled (pixel) space.
    let transform = kurbo::Affine::translate((
        (run.origin.x * scale) as f64,
        (run.origin.y * scale) as f64,
    ));

    let brush = crate::color::to_brush(&run.color);

    // Map layout GlyphEntry → vello::Glyph.  Positions are in the run's local
    // coordinate space; the transform above handles the run origin.
    let glyphs = run.glyphs.iter().map(|g| vello::Glyph {
        id: g.id as u32,
        x: g.x * scale,
        y: g.y * scale,
    });

    // Empty normalized coords = non-variable (static) font.
    let coords: &[vello::NormalizedCoord] = &[];

    scene
        .draw_glyphs(font)
        .font_size(run.font_size * scale)
        .transform(transform)
        .glyph_transform(None)
        .normalized_coords(coords)
        .brush(&brush)
        .hint(false)
        .draw(peniko::Fill::NonZero, glyphs);
}
