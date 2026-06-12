// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Emitting `PositionedItem`s from Parley glyph runs.
//!
//! Converts Parley's positioned glyph runs into the renderer-agnostic
//! [`PositionedItem`] values consumed by `loki-vello`.

use std::sync::Arc;

use parley::PositionedLayoutItem;

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutPoint, LayoutRect};
use crate::items::{
    DecorationKind, GlyphEntry, GlyphSynthesis, PositionedBorderRect, PositionedDecoration,
    PositionedGlyphRun, PositionedItem, PositionedRect,
};

use super::span_helpers::{
    span_has_shadow, span_highlight_for_range, span_link_url_for_range,
    span_vertical_align_for_range,
};
use super::types::{ResolvedParaProps, StyleSpan, VerticalAlign};

#[cfg(debug_assertions)]
use super::span_helpers::span_font_name_for_range;

/// Walk Parley lines and emit glyph runs, highlights, shadows, and decorations
/// into `items`.
pub(super) fn collect_glyph_runs(
    layout: &parley::Layout<LayoutColor>,
    clean_text: &str,
    clean_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    resources: &mut FontResources,
    items: &mut Vec<PositionedItem>,
) {
    for (line_index, line) in layout.lines().enumerate() {
        let indent_x = if line_index == 0 && para_props.indent_hanging > 0.0 {
            para_props.indent_start - para_props.indent_hanging
        } else {
            para_props.indent_start
        };
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let run = glyph_run.run();
            let style = glyph_run.style();
            let run_offset = glyph_run.offset();
            let run_baseline = glyph_run.baseline();
            let text_range = run.text_range();

            let raw_bytes: &[u8] = run.font().data.data();
            let font_data = resources
                .font_data_cache
                .entry(raw_bytes.as_ptr() as u64)
                .or_insert_with(|| Arc::new(raw_bytes.to_vec()))
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

            #[cfg(debug_assertions)]
            {
                let font_name = span_font_name_for_range(clean_spans, text_range.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                if font_name.contains("Calibri") || font_name.contains("calibri") {
                    eprintln!(
                        "CALIBRI LAYOUT RUN: font={}, font_size={}, advance={}, glyph_count={}, text={:?}",
                        font_name,
                        run.font_size(),
                        glyph_run.advance(),
                        glyphs.len(),
                        &clean_text[text_range.clone()]
                    );
                }
            }

            let link_url = span_link_url_for_range(clean_spans, text_range.clone());
            let va_offset = span_vertical_align_for_range(clean_spans, text_range.clone())
                .map(|(va, orig_size)| match va {
                    VerticalAlign::Superscript => -orig_size * 0.35,
                    VerticalAlign::Subscript => orig_size * 0.20,
                })
                .unwrap_or(0.0);

            // Highlight rect (gap #10) — emitted before glyph run.
            if let Some(hl_color) = span_highlight_for_range(clean_spans, text_range.clone()) {
                let m = run.metrics();
                items.push(PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(
                        run_offset + indent_x,
                        run_baseline - m.ascent + va_offset,
                        glyph_run.advance(),
                        m.ascent + m.descent,
                    ),
                    color: hl_color,
                }));
            }

            // Shadow copy (gap #24).
            // TODO(shadow): replace with Vello blur filter for soft shadow.
            if span_has_shadow(clean_spans, text_range.clone()) {
                items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
                    origin: LayoutPoint {
                        x: run_offset + indent_x + 0.5,
                        y: run_baseline + va_offset + 0.5,
                    },
                    font_data: font_data.clone(),
                    font_index: run.font().index,
                    font_size: run.font_size(),
                    glyphs: glyphs.clone(),
                    color: LayoutColor::new(0.4, 0.4, 0.4, 1.0),
                    synthesis: GlyphSynthesis {
                        bold: synthesis.embolden(),
                        italic: synthesis.skew().is_some(),
                    },
                    link_url: None,
                }));
            }

            // Main glyph run.
            items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
                origin: LayoutPoint {
                    x: run_offset + indent_x,
                    y: run_baseline + va_offset,
                },
                font_data,
                font_index: run.font().index,
                font_size: run.font_size(),
                glyphs,
                color: style.brush,
                synthesis: GlyphSynthesis {
                    bold: synthesis.embolden(),
                    italic: synthesis.skew().is_some(),
                },
                link_url,
            }));

            // Underline decoration.
            if let Some(deco) = &style.underline {
                let m = run.metrics();
                // COMPAT(parley-0.6): Y-up → Y-down negation for offsets.
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: run_offset + indent_x,
                    y: run_baseline - deco.offset.unwrap_or(m.underline_offset),
                    width: glyph_run.advance(),
                    thickness: deco.size.unwrap_or(m.underline_size),
                    kind: DecorationKind::Underline,
                    color: deco.brush,
                }));
            }

            // Strikethrough decoration.
            if let Some(deco) = &style.strikethrough {
                let m = run.metrics();
                // COMPAT(parley-0.6): same Y-up → Y-down negation as underline.
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: run_offset + indent_x,
                    y: run_baseline - deco.offset.unwrap_or(m.strikethrough_offset),
                    width: glyph_run.advance(),
                    thickness: deco.size.unwrap_or(m.strikethrough_size),
                    kind: DecorationKind::Strikethrough,
                    color: deco.brush,
                }));
            }
        }
    }
}

/// Prepend border and background fill items at index 0 (renders below glyphs).
pub(super) fn prepend_border_and_background(
    items: &mut Vec<PositionedItem>,
    para_props: &ResolvedParaProps,
    total_width: f32,
    total_height: f32,
) {
    let has_border = para_props.border_top.is_some()
        || para_props.border_right.is_some()
        || para_props.border_bottom.is_some()
        || para_props.border_left.is_some();
    if has_border {
        let bw = total_width + para_props.indent_start + para_props.indent_end;
        items.insert(
            0,
            PositionedItem::BorderRect(PositionedBorderRect {
                rect: LayoutRect::new(0.0, 0.0, bw, total_height),
                top: para_props.border_top,
                right: para_props.border_right,
                bottom: para_props.border_bottom,
                left: para_props.border_left,
            }),
        );
    }
    if let Some(bg) = para_props.background_color {
        let bw = total_width + para_props.indent_start + para_props.indent_end;
        items.insert(
            0,
            PositionedItem::FilledRect(PositionedRect {
                rect: LayoutRect::new(0.0, 0.0, bw, total_height),
                color: bg,
            }),
        );
    }
}
