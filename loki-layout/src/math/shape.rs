// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Token shaping for the math typesetter.
//!
//! Each MathML token (`mi`/`mn`/`mo`/`mtext`) is shaped as a one-off Parley
//! layout so its glyphs and metrics can be composed manually by
//! [`super::compose`]. The extracted glyph run mirrors the main paragraph
//! extraction in [`crate::para`], but the run origin is placed on the box
//! baseline (`y = 0`) so composition can stack and shift boxes freely.

use std::borrow::Cow;
use std::sync::Arc;

use parley::{FontFamily, FontStyle, PositionedLayoutItem, StyleProperty};

/// Font stack for mathematics: a real math font when present (Word uses Cambria
/// Math), falling back to the generic `serif` face. Math is conventionally set
/// in a serif/math face, not the sans-serif body default.
const MATH_FONT_STACK: &str = "Cambria Math, STIX Two Math, Latin Modern Math, serif";

use super::MBox;
use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::LayoutPoint;
use crate::items::{GlyphEntry, GlyphSynthesis, PositionedGlyphRun, PositionedItem};

/// Shapes `text` at `font_size` into a baseline-relative [`MBox`] (a single
/// glyph atom). `italic` selects the italic face (used for identifiers).
pub(super) fn shape_token(
    resources: &mut FontResources,
    text: &str,
    font_size: f32,
    italic: bool,
    color: LayoutColor,
    display_scale: f32,
) -> MBox {
    if text.is_empty() {
        return MBox::empty();
    }

    let mut builder =
        resources
            .layout_cx
            .ranged_builder(&mut resources.font_cx, text, display_scale, true);
    builder.push_default(StyleProperty::Brush(color));
    builder.push_default(StyleProperty::FontSize(font_size));
    // Math is conventionally set in a serif/math face (Word uses Cambria Math),
    // not the sans-serif body default. Request the generic serif family so the
    // platform's serif face is used (a real math font if it is the serif
    // default); falls back cleanly cross-platform.
    builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
        Cow::Borrowed(MATH_FONT_STACK),
    )));
    if italic {
        builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
    }
    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    let width = layout.width();
    let mut ascent = 0.0f32;
    let mut descent = 0.0f32;
    let mut items: Vec<PositionedItem> = Vec::new();

    if let Some(line) = layout.lines().next() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            let metrics = run.metrics();
            ascent = ascent.max(metrics.ascent);
            descent = descent.max(metrics.descent);

            let run_offset = glyph_run.offset();
            let run_baseline = glyph_run.baseline();
            let raw: &[u8] = run.font().data.data();
            let font_data = resources
                .font_data_cache
                .entry(raw.as_ptr() as u64)
                .or_insert_with(|| Arc::new(raw.to_vec()))
                .clone();
            let synthesis = run.synthesis();
            let glyphs: Vec<GlyphEntry> = glyph_run
                .positioned_glyphs()
                .map(|g| GlyphEntry {
                    id: g.id as u16,
                    x: g.x - run_offset,
                    y: g.y - run_baseline,
                    advance: g.advance,
                })
                .collect();

            items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
                // Baseline sits at y = 0; composition shifts the whole box.
                origin: LayoutPoint {
                    x: run_offset,
                    y: 0.0,
                },
                font_data,
                font_index: run.font().index,
                font_size: run.font_size(),
                glyphs,
                color,
                synthesis: GlyphSynthesis {
                    bold: synthesis.embolden(),
                    italic: synthesis.skew().is_some(),
                },
                normalized_coords: run.normalized_coords().to_vec(),
                link_url: None,
            }));
        }
    }

    MBox {
        width,
        ascent,
        descent,
        items,
    }
}

/// Shapes `ch` scaled up so its visual height is at least `target_height`,
/// producing a "stretched" delimiter or radical sign. Uniform glyph scaling is
/// an approximation of a true extensible glyph — the sign also widens — but it
/// keeps the symbol's shape and needs no special renderer support. The glyph is
/// never shrunk below its natural size, and the scale is capped to avoid
/// pathological growth.
pub(super) fn stretchy_glyph(
    resources: &mut FontResources,
    ch: &str,
    font_size: f32,
    target_height: f32,
    color: LayoutColor,
    display_scale: f32,
) -> MBox {
    let base = shape_token(resources, ch, font_size, false, color, display_scale);
    let natural = base.ascent + base.descent;
    if natural <= 0.0 || target_height <= natural * 1.05 {
        return base;
    }
    let factor = (target_height / natural).min(6.0);
    shape_token(
        resources,
        ch,
        font_size * factor,
        false,
        color,
        display_scale,
    )
}
