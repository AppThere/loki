// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Token shaping for the math typesetter.
//!
//! Each MathML token (`mi`/`mn`/`mo`/`mtext`) is shaped as a one-off Parley
//! layout so its glyphs and metrics can be composed manually by
//! [`super::compose`]. The extracted glyph run mirrors the main paragraph
//! extraction in [`crate::para`], but the run origin is placed on the box
//! baseline (`y = 0`) so composition can stack and shift boxes freely.

use std::sync::Arc;

use parley::{FontStyle, PositionedLayoutItem, StyleProperty};

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
