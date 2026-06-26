// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Glyph-run emission shared by the main paragraph layout loop
//! ([`crate::para`]) and the banded (drop-cap / float wrap) layout path
//! ([`crate::para_band`]).
//!
//! Given one Parley [`parley::GlyphRun`] and the horizontal offset of its line,
//! this emits the run's highlight underlay, hard-shadow copy, main glyph run,
//! and underline/strikethrough decorations as renderer-agnostic
//! [`PositionedItem`]s. The y coordinates are the run's native layout-space
//! values; callers that stack a second sub-layout translate the emitted items
//! vertically afterwards.

use std::sync::Arc;

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutPoint, LayoutRect};
use crate::items::{
    DecorationKind, GlyphEntry, GlyphSynthesis, PositionedDecoration, PositionedGlyphRun,
    PositionedItem, PositionedRect,
};
use crate::para::{
    StyleSpan, VerticalAlign, span_has_shadow, span_highlight_for_range, span_link_url_for_range,
    span_vertical_align_for_range,
};

/// Emits one shaped glyph run at horizontal offset `indent_x`, appending the
/// highlight, shadow, glyph, and decoration items to `items`.
///
/// `spans` supplies per-range character styling (highlight, link, shadow,
/// super/subscript) looked up by the run's text range.
///
/// `scale` is the horizontal text scale (OOXML `w:w` / ODF `style:text-scale`);
/// `1.0` = no scaling. Glyph advances and within-run x positions are multiplied
/// by `scale`, anchored at the run's left edge, and the highlight/decoration
/// widths follow. The caller is responsible for shifting later runs on the line
/// by the extra `(scale - 1) * advance` width (see the call site in
/// [`crate::para`]). COMPAT(parley-0.6): Parley has no geometric horizontal
/// scale, so the unscaled run width is what drove line-breaking.
pub(crate) fn emit_glyph_run(
    glyph_run: &parley::GlyphRun<'_, LayoutColor>,
    indent_x: f32,
    spans: &[StyleSpan],
    scale: f32,
    resources: &mut FontResources,
    items: &mut Vec<PositionedItem>,
    // When `true`, emit the per-run highlight underlay (used by the banded
    // drop-cap / float path). The main paragraph path passes `false` and emits
    // highlights via a Parley selection-geometry pass instead (inline in
    // [`crate::para::layout_paragraph`]), which is robust to Parley coalescing
    // adjacent runs that differ only in highlight colour — an attribute Parley
    // does not track.
    emit_highlight: bool,
) {
    let run = glyph_run.run();
    let style = glyph_run.style();
    let run_offset = glyph_run.offset();
    let run_baseline = glyph_run.baseline();

    // Intern the font data bytes by pointer identity so all glyph runs using the
    // same Parley-internal font share the same Arc. Without this, every run
    // would clone the full font file bytes (potentially hundreds of KB)
    // producing unique Arc pointers that defeat the FontDataCache in loki-vello.
    let raw_bytes: &[u8] = run.font().data.data();
    let font_data = resources
        .font_data_cache
        .entry(raw_bytes.as_ptr() as u64)
        .or_insert_with(|| Arc::new(raw_bytes.to_vec()))
        .clone();
    let synthesis = run.synthesis();
    // Horizontal scale (w:w) stretches each glyph's within-run x offset and its
    // advance, anchored at the run's left edge (local x 0). y is untouched.
    let glyphs: Vec<GlyphEntry> = glyph_run
        .positioned_glyphs()
        .map(|g| GlyphEntry {
            id: g.id as u16,
            x: (g.x - run_offset) * scale,
            y: g.y - run_baseline,
            advance: g.advance * scale,
        })
        .collect();
    // Scaled width of the run, used for highlight and decoration extents.
    let scaled_advance = glyph_run.advance() * scale;

    let text_range = run.text_range();

    let link_url = span_link_url_for_range(spans, text_range.clone());

    // ── Vertical offset for super/subscript (gap #3) ──────────────────────────
    // Parley does not expose baseline-shift, so font size is reduced to 58 % in
    // push_para_styles. We manually shift the run origin here so the text
    // actually appears above/below the baseline.
    // Superscript: raise by 35 % of the original (pre-reduction) font size.
    // Subscript:   lower by 20 % of the original font size.
    let va_offset = span_vertical_align_for_range(spans, text_range.clone())
        .map(|(va, orig_size)| match va {
            VerticalAlign::Superscript => -orig_size * 0.35,
            VerticalAlign::Subscript => orig_size * 0.20,
        })
        .unwrap_or(0.0);

    // ── Highlight colour (gap #10) ────────────────────────────────────────────
    // Emit a filled rect sized to the run's ink extent BEFORE the glyph run so
    // the background renders below the text. Only on the banded path; the main
    // path handles highlights via a selection-geometry pass (robust to coalescing).
    if emit_highlight && let Some(hl_color) = span_highlight_for_range(spans, text_range.clone()) {
        let m = run.metrics();
        items.push(PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(
                run_offset + indent_x,
                run_baseline - m.ascent + va_offset,
                scaled_advance,
                m.ascent + m.descent,
            ),
            color: hl_color,
        }));
    }

    // ── Shadow copy (gap #24) ─────────────────────────────────────────────────
    // Emit a dark-grey copy of the run offset by (0.5 pt, 0.5 pt) so it appears
    // as a hard shadow behind the main run.
    // TODO(shadow): replace with Vello blur filter for soft shadow once
    // scene.rs blur pipeline is verified stable (see TODO in scene.rs).
    if span_has_shadow(spans, text_range.clone()) {
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
            link_url: None, // shadows don't carry link metadata
        }));
    }

    // ── Main glyph run ────────────────────────────────────────────────────────
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
        // COMPAT(parley-0.6): RunMetrics offsets follow OpenType / skrifa Y-up
        // convention (negative = below baseline). Negate to convert to screen
        // Y-down (positive = below baseline).
        items.push(PositionedItem::Decoration(PositionedDecoration {
            x: run_offset + indent_x,
            y: run_baseline - deco.offset.unwrap_or(m.underline_offset),
            width: scaled_advance,
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
            width: scaled_advance,
            thickness: deco.size.unwrap_or(m.strikethrough_size),
            kind: DecorationKind::Strikethrough,
            color: deco.brush,
        }));
    }
}
