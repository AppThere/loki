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

use std::ops::Range;
use std::sync::Arc;

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutPoint, LayoutRect};
use crate::items::{
    DecorationKind, DecorationStyle, GlyphEntry, GlyphSynthesis, PositionedDecoration,
    PositionedGlyphRun, PositionedItem, PositionedRect,
};
use crate::para::{StrikethroughStyle, StyleSpan, UnderlineStyle, VerticalAlign, span_at_offset};

pub(crate) fn underline_deco_style(u: UnderlineStyle) -> DecorationStyle {
    match u {
        UnderlineStyle::Single => DecorationStyle::Solid,
        UnderlineStyle::Double => DecorationStyle::Double,
        UnderlineStyle::Dotted => DecorationStyle::Dotted,
        UnderlineStyle::Dash => DecorationStyle::Dashed,
        UnderlineStyle::Wave => DecorationStyle::Wave,
        UnderlineStyle::Thick => DecorationStyle::Thick,
    }
}

/// The first span fully containing `r` — resolved once per glyph run and read
/// field-by-field by every per-run attribute lookup in [`emit_glyph_run`].
fn span_covering_range(spans: &[StyleSpan], r: Range<usize>) -> Option<&StyleSpan> {
    spans
        .iter()
        .find(|s| s.range.start <= r.start && s.range.end >= r.end)
}

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
) -> f32 {
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

    // ── Per-glyph geometry: horizontal scale (w:w) + baseline shift (w:position)
    // Both are post-shaping attributes Parley does not know about, so a run that
    // mixes them with a same-font/colour neighbour is shaped into ONE glyph run.
    // We therefore resolve each glyph's span via its cluster's source byte offset
    // and apply scale / rise per glyph (anchored at the run's left edge), instead
    // of a single per-run lookup that the whole coalesced run would miss.
    //
    // `glyph_text_offsets[i]` is the source byte offset of the i-th glyph of the
    // PARENT run; it aligns 1:1 with `glyph_run.glyphs()` only when this glyph
    // run covers the whole run (exactly the coalescing case we must fix). When it
    // does not (Parley split the run on a real style change), we fall back to the
    // caller's per-run `scale` and no extra rise — unchanged behaviour.
    //
    // The cluster/glyph vectors below are built only when some span carries a
    // scale or baseline shift (three avoided heap allocations on the common
    // no-`w:w`/no-`w:position` path). The per-glyph path accumulates scaled
    // advances; the uniform fast path reproduces the previous geometry exactly.
    let has_per_glyph = spans
        .iter()
        .any(|s| s.scale.is_some() || s.baseline_shift.is_some());
    let (glyphs, scaled_advance): (Vec<GlyphEntry>, f32) = 'geom: {
        if has_per_glyph {
            let glyph_text_offsets: Vec<usize> = run
                .visual_clusters()
                .flat_map(|c| {
                    let start = c.text_range().start;
                    c.glyphs().map(move |_| start)
                })
                .collect();
            let raw_glyphs: Vec<parley::Glyph> = glyph_run.glyphs().collect();
            let aligned = glyph_text_offsets.len() == raw_glyphs.len();
            // (scale, rise) per glyph.
            let per_glyph: Vec<(f32, f32)> = raw_glyphs
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    if aligned {
                        let s = span_at_offset(spans, glyph_text_offsets[i]);
                        (
                            s.and_then(|sp| sp.scale).unwrap_or(scale),
                            s.and_then(|sp| sp.baseline_shift).unwrap_or(0.0),
                        )
                    } else {
                        (scale, 0.0)
                    }
                })
                .collect();
            let uniform = per_glyph
                .iter()
                .all(|&(sc, rise)| (sc - scale).abs() < f32::EPSILON && rise == 0.0);
            if !uniform {
                let mut pen = 0.0f32;
                let glyphs = raw_glyphs
                    .iter()
                    .enumerate()
                    .map(|(i, g)| {
                        let (sc, rise) = per_glyph[i];
                        let entry = GlyphEntry {
                            id: g.id as u16,
                            x: pen + g.x * sc,
                            // `rise` raises the glyph (screen-y is down, so subtract).
                            y: g.y - rise,
                            advance: g.advance * sc,
                        };
                        pen += g.advance * sc;
                        entry
                    })
                    .collect();
                break 'geom (glyphs, pen);
            }
        }
        let glyphs = glyph_run
            .positioned_glyphs()
            .map(|g| GlyphEntry {
                id: g.id as u16,
                x: (g.x - run_offset) * scale,
                y: g.y - run_baseline,
                advance: g.advance * scale,
            })
            .collect();
        (glyphs, glyph_run.advance() * scale)
    };

    let covering_span = span_covering_range(spans, run.text_range());

    let link_url = covering_span.and_then(|s| s.link_url.clone());

    // ── Vertical offset for super/subscript (gap #3) ──────────────────────────
    // Parley does not expose baseline-shift, so font size is reduced to 58 % in
    // push_para_styles. We manually shift the run origin here so the text
    // actually appears above/below the baseline.
    // Superscript: raise by 35 % of the original (pre-reduction) font size.
    // Subscript:   lower by 20 % of the original font size.
    let va_offset = covering_span
        .and_then(|s| s.vertical_align.map(|va| (va, s.font_size)))
        .map(|(va, orig_size)| match va {
            VerticalAlign::Superscript => -orig_size * 0.35,
            VerticalAlign::Subscript => orig_size * 0.20,
        })
        .unwrap_or(0.0);

    // ── Highlight colour (gap #10) ────────────────────────────────────────────
    // Emit a filled rect sized to the run's ink extent BEFORE the glyph run so
    // the background renders below the text. Only on the banded path; the main
    // path handles highlights via a selection-geometry pass (robust to coalescing).
    if emit_highlight && let Some(hl_color) = covering_span.and_then(|s| s.highlight_color) {
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
    if covering_span.is_some_and(|s| s.shadow) {
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
            normalized_coords: run.normalized_coords().to_vec(),
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
        // VF instance (e.g. Arimo wght=700 for bold Arial); empty for static faces.
        normalized_coords: run.normalized_coords().to_vec(),
        link_url,
    }));

    // Underline decoration. Parley supplies the geometry (offset/size) but not
    // the `w:u` variant, so recover it from our spans by the run's text range.
    if let Some(deco) = &style.underline {
        let m = run.metrics();
        let deco_style = covering_span
            .and_then(|s| s.underline)
            .map(underline_deco_style)
            .unwrap_or(DecorationStyle::Solid);
        // COMPAT(parley-0.6): RunMetrics offsets follow OpenType / skrifa Y-up
        // convention (negative = below baseline). Negate to convert to screen
        // Y-down (positive = below baseline).
        items.push(PositionedItem::Decoration(PositionedDecoration {
            x: run_offset + indent_x,
            y: run_baseline - deco.offset.unwrap_or(m.underline_offset),
            width: scaled_advance,
            thickness: deco.size.unwrap_or(m.underline_size),
            kind: DecorationKind::Underline,
            style: deco_style,
            color: deco.brush,
        }));
    }

    // Strikethrough decoration (single, or `w:dstrike` double).
    if let Some(deco) = &style.strikethrough {
        let m = run.metrics();
        let deco_style = match covering_span.and_then(|s| s.strikethrough) {
            Some(StrikethroughStyle::Double) => DecorationStyle::Double,
            _ => DecorationStyle::Solid,
        };
        // COMPAT(parley-0.6): same Y-up → Y-down negation as underline.
        items.push(PositionedItem::Decoration(PositionedDecoration {
            x: run_offset + indent_x,
            y: run_baseline - deco.offset.unwrap_or(m.strikethrough_offset),
            width: scaled_advance,
            thickness: deco.size.unwrap_or(m.strikethrough_size),
            kind: DecorationKind::Strikethrough,
            style: deco_style,
            color: deco.brush,
        }));
    }

    // Extra horizontal width this run added beyond its natural (unscaled)
    // advance, so the caller can shift later runs on the line by the same amount
    // (covers both uniform `w:w` scaling and a coalesced run with a scaled
    // sub-region).
    scaled_advance - glyph_run.advance()
}
