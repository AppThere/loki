// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Banded paragraph layout — per-line precision for drop caps and floats.
//!
//! A *band* is a rectangular region on one side of the first part of a
//! paragraph that the body text must avoid: the dropped initial of a drop cap,
//! or a floating image the text wraps around. Parley's public API exposes no
//! per-line max-advance (the setter lives on its private breaker state), so the
//! lines beside the band and the lines below it cannot be broken at two widths
//! in one layout. This module instead lays out the body **twice**: the lines
//! overlapping the band are taken from a narrow layout, and the remaining text
//! is re-flowed at full width and stacked below — so text below the band
//! reclaims the full column, matching Word.
//!
//! Glyph emission is shared with the main paragraph path via
//! [`crate::para_emit::emit_glyph_run`]. The banded path is only used for
//! plain text (no tabs / inline math), which a drop-cap / float paragraph is.

use parley::{AlignmentOptions, PositionedLayoutItem};

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::items::PositionedItem;
use crate::para::{ResolvedParaProps, StyleSpan, push_para_styles};
use crate::para_emit::emit_glyph_run;

/// A side band the first lines of a paragraph must clear.
pub(crate) struct Band {
    /// Horizontal width (points) the band occupies, including any gap.
    pub inset: f32,
    /// Vertical extent (points) the band covers from the paragraph top; lines
    /// whose top is above this are narrowed, lines below reclaim full width.
    pub cover_height: f32,
    /// `true` when the band is on the **left** (object on the left, text shifted
    /// right); `false` when on the right (text narrowed but not shifted).
    pub shift_text: bool,
}

/// The laid-out body of a banded paragraph.
pub(crate) struct BandBody {
    pub items: Vec<PositionedItem>,
    pub height: f32,
    pub width: f32,
    pub first_baseline: f32,
    pub last_baseline: f32,
    pub line_boundaries: Vec<(f32, f32)>,
}

/// Lays out `text` with a leading side band: the lines overlapping the band are
/// broken at the narrowed width (and shifted right when the band is on the
/// left), and the remaining text is re-flowed at full width below.
pub(crate) fn layout_band_body(
    resources: &mut FontResources,
    text: &str,
    spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    line_w: f32,
    display_scale: f32,
    band: &Band,
) -> BandBody {
    let indent_start = para_props.indent_start;
    let x_shift = if band.shift_text { band.inset } else { 0.0 };
    let narrow_w = (line_w - band.inset).max(1.0);

    let narrow = build_layout(resources, text, spans, para_props, narrow_w, display_scale);

    // How many leading lines overlap the band vertically.
    let total = narrow.lines().count();
    let n_lines = narrow
        .lines()
        .take_while(|l| l.metrics().block_min_coord < band.cover_height)
        .count()
        .min(total);

    let mut items = Vec::new();

    // Whole body fits within the band → one narrow segment.
    if n_lines >= total {
        emit_lines(
            &narrow,
            spans,
            resources,
            indent_start + x_shift,
            0,
            usize::MAX,
            &mut items,
        );
        let (first_baseline, last_baseline) = baselines(&narrow, 0.0);
        return BandBody {
            items,
            height: narrow.height(),
            width: narrow.width() + x_shift,
            first_baseline,
            last_baseline,
            line_boundaries: line_boundaries(&narrow, 0.0, 0, usize::MAX),
        };
    }

    // Split: emit band lines [0, n_lines) narrow (+shift); reflow the tail full.
    let split_byte = narrow
        .lines()
        .nth(n_lines)
        .map(|l| l.text_range().start)
        .unwrap_or(text.len());
    let band_height = narrow
        .lines()
        .nth(n_lines)
        .map(|l| l.metrics().block_min_coord)
        .unwrap_or_else(|| narrow.height());

    emit_lines(
        &narrow,
        spans,
        resources,
        indent_start + x_shift,
        0,
        n_lines,
        &mut items,
    );
    let mut boundaries = line_boundaries(&narrow, 0.0, 0, n_lines);
    let first_baseline = narrow
        .lines()
        .next()
        .map(|l| l.metrics().baseline)
        .unwrap_or(0.0);
    let narrow_width = narrow.width() + x_shift;
    drop(narrow);

    let (suffix_text, suffix_spans) = crate::para_drop_cap::trim_leading(text, spans, split_byte);
    let full = build_layout(
        resources,
        &suffix_text,
        &suffix_spans,
        para_props,
        line_w,
        display_scale,
    );

    let tail_start = items.len();
    emit_lines(
        &full,
        &suffix_spans,
        resources,
        indent_start,
        0,
        usize::MAX,
        &mut items,
    );
    for it in &mut items[tail_start..] {
        it.translate(0.0, band_height);
    }
    boundaries.extend(line_boundaries(&full, band_height, 0, usize::MAX));
    let last_baseline = full
        .lines()
        .last()
        .map(|l| l.metrics().baseline)
        .unwrap_or(0.0)
        + band_height;

    BandBody {
        items,
        height: band_height + full.height(),
        width: narrow_width.max(full.width()),
        first_baseline,
        last_baseline,
        line_boundaries: boundaries,
    }
}

/// Builds and breaks a plain-text layout at `max_w`, applying the paragraph
/// alignment.
fn build_layout(
    resources: &mut FontResources,
    text: &str,
    spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    max_w: f32,
    display_scale: f32,
) -> parley::Layout<LayoutColor> {
    let mut builder =
        resources
            .layout_cx
            .ranged_builder(&mut resources.font_cx, text, display_scale, true);
    push_para_styles(&mut builder, para_props, spans);
    let mut layout = builder.build(text);
    layout.break_all_lines(Some(max_w));
    layout.align(para_props.alignment, AlignmentOptions::default());
    layout
}

/// Emits glyph runs for lines `[lo, hi)` of `layout` at horizontal offset
/// `indent_x`.
fn emit_lines(
    layout: &parley::Layout<LayoutColor>,
    spans: &[StyleSpan],
    resources: &mut FontResources,
    indent_x: f32,
    lo: usize,
    hi: usize,
    items: &mut Vec<PositionedItem>,
) {
    for (i, line) in layout.lines().enumerate() {
        if i < lo || i >= hi {
            continue;
        }
        // Reserve width added by horizontally-scaled (w:w) runs so later runs on
        // the line do not overlap (see the call site in `para`). Reset per line.
        let mut extra_x = 0.0f32;
        for item in line.items() {
            if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                let scale = crate::para::span_scale_for_range(spans, glyph_run.run().text_range())
                    .unwrap_or(1.0);
                extra_x += emit_glyph_run(
                    &glyph_run,
                    indent_x + extra_x,
                    spans,
                    scale,
                    resources,
                    items,
                    // Banded path keeps the per-run highlight underlay.
                    true,
                );
            }
        }
    }
}

/// Per-line `(min, max)` block coordinates for lines `[lo, hi)`, offset by `dy`.
fn line_boundaries(
    layout: &parley::Layout<LayoutColor>,
    dy: f32,
    lo: usize,
    hi: usize,
) -> Vec<(f32, f32)> {
    layout
        .lines()
        .enumerate()
        .filter(|(i, _)| *i >= lo && *i < hi)
        .map(|(_, l)| {
            let m = l.metrics();
            (m.block_min_coord + dy, m.block_max_coord + dy)
        })
        .collect()
}

/// First and last line baselines, offset by `dy`.
fn baselines(layout: &parley::Layout<LayoutColor>, dy: f32) -> (f32, f32) {
    let first = layout
        .lines()
        .next()
        .map(|l| l.metrics().baseline)
        .unwrap_or(0.0)
        + dy;
    let last = layout
        .lines()
        .last()
        .map(|l| l.metrics().baseline)
        .unwrap_or(0.0)
        + dy;
    (first, last)
}

#[cfg(test)]
#[path = "para_band_tests.rs"]
mod tests;
