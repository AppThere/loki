// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Dropped-initial (drop cap) layout.
//!
//! A drop cap enlarges the first character(s) of a paragraph so the initial
//! spans several body lines, with the body text flowing beside it. This module
//! sizes and positions the enlarged initial and reports the horizontal region
//! the first lines of body text must avoid. The caller (`layout_paragraph`)
//! drives Parley's per-line breaker to narrow + shift those lines and appends
//! the cap glyph(s) produced here.
//!
//! OOXML `w:framePr`/`w:dropCap`; ODF `style:drop-cap`. See
//! [`loki_doc_model::style::props::drop_cap::DropCap`].

use parley::{
    AlignmentOptions, FontFamily, FontStyle, FontWeight, PositionedLayoutItem, StyleProperty,
};

use loki_doc_model::style::props::drop_cap::{DropCap, DropCapLength};

use crate::font::FontResources;
use crate::geometry::LayoutPoint;
use crate::items::{GlyphEntry, GlyphSynthesis, PositionedGlyphRun, PositionedItem};
use crate::para::StyleSpan;

/// The result of planning a dropped initial for one paragraph.
pub(crate) struct DropCapPlan {
    /// Number of body lines the cap spans (`>= 1`).
    pub n_lines: usize,
    /// Horizontal inset (cap advance + distance), in points, that the first
    /// `n_lines` body lines must leave clear on the left. `0.0` in margin mode
    /// (the cap hangs in the margin and the body is not inset).
    pub body_inset: f32,
    /// Cap glyph draw items in paragraph-local space (the body's `indent_start`
    /// is added by the caller, as for body glyph runs).
    pub items: Vec<PositionedItem>,
    /// Lowest `y` reached by the cap ink, for paragraph-height growth.
    pub bottom: f32,
}

/// Returns the byte length of the leading initial to enlarge, per `length`.
///
/// `Chars(n)` takes the first `n` Unicode scalar values; `Word` takes up to the
/// first whitespace. Returns `0` when the paragraph has no usable initial.
pub(crate) fn cap_byte_len(text: &str, length: DropCapLength) -> usize {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return 0;
    }
    // Account for any leading whitespace skipped by `trim_start`.
    let lead_ws = text.len() - trimmed.len();
    match length {
        DropCapLength::Word => {
            let word_len = trimmed
                .char_indices()
                .find(|(_, c)| c.is_whitespace())
                .map(|(i, _)| i)
                .unwrap_or(trimmed.len());
            lead_ws + word_len
        }
        DropCapLength::Chars(n) => {
            let n = n.max(1) as usize;
            let end = trimmed
                .char_indices()
                .nth(n)
                .map(|(i, _)| i)
                .unwrap_or(trimmed.len());
            lead_ws + end
        }
        // Unknown future length kinds degenerate to a single character.
        _ => lead_ws + trimmed.chars().next().map(char::len_utf8).unwrap_or(0),
    }
}

/// Removes the leading `k` bytes (the extracted initial) from `text` and shifts
/// `spans` to match, dropping spans that lay wholly within the removed prefix
/// and clamping any that straddle it. `k` must be a char boundary.
///
/// Only used on the read-only paint path, where the original byte indices are
/// not needed for hit-testing (the Parley layout is not retained), so trimming
/// the body text is lossless for rendering purposes.
pub(crate) fn trim_leading(text: &str, spans: &[StyleSpan], k: usize) -> (String, Vec<StyleSpan>) {
    let body = text[k..].to_string();
    let body_len = body.len();
    let spans = spans
        .iter()
        .filter_map(|s| {
            if s.range.end <= k {
                return None; // entirely within the removed initial
            }
            let start = s.range.start.saturating_sub(k).min(body_len);
            let end = s.range.end.saturating_sub(k).min(body_len);
            let mut s2 = s.clone();
            s2.range = start..end;
            Some(s2)
        })
        .collect();
    (body, spans)
}

/// Plans the dropped initial: sizes the cap to span `dc.lines` rows, positions
/// it against the first body line, and returns its glyph items plus the body
/// inset. Returns `None` if the cap cannot be shaped (empty or zero advance).
///
/// `body_line_height` is the body's line pitch; `first_baseline`/`first_ascent`
/// come from the body layout's first line. `cap_text` is the already-extracted
/// initial. `base` supplies the cap's font family / weight / style / colour.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_drop_cap(
    resources: &mut FontResources,
    cap_text: &str,
    base: &StyleSpan,
    dc: &DropCap,
    body_line_height: f32,
    first_baseline: f32,
    first_ascent: f32,
    display_scale: f32,
) -> Option<DropCapPlan> {
    let n_lines = (dc.lines as usize).max(1);
    let cap_text = cap_text.trim();
    if cap_text.is_empty() || body_line_height <= 0.0 {
        return None;
    }

    // Probe at a one-line size to measure the font's ascent ratio, then scale so
    // the cap's ascent spans `n_lines` rows (Word sizes the initial to the line
    // band it occupies).
    let probe_size = body_line_height.max(1.0);
    let probe = shape_cap(resources, cap_text, base, probe_size, display_scale)?;
    if probe.ascent <= 0.0 {
        return None;
    }
    let target_ascent = n_lines as f32 * body_line_height;
    let cap_size = (probe_size * target_ascent / probe.ascent).max(1.0);

    let shaped = shape_cap(resources, cap_text, base, cap_size, display_scale)?;
    if shaped.advance <= 0.0 {
        return None;
    }

    let distance = pts(dc.distance);
    // Align the cap's top with the top of the first body line; its baseline then
    // sits `cap_ascent` below that. (line0 top = first_baseline − first_ascent.)
    let line0_top = first_baseline - first_ascent;
    let cap_baseline = line0_top + shaped.ascent;

    // Margin mode: the cap hangs in the left margin and the body is not inset.
    // Drop (in-text) mode: the body's first `n_lines` lines clear the cap.
    let (cap_x, body_inset) = if dc.margin {
        (-(shaped.advance + distance), 0.0)
    } else {
        (0.0, shaped.advance + distance)
    };

    let mut items = shaped.items;
    for item in &mut items {
        item.translate(cap_x, cap_baseline);
    }
    let bottom = cap_baseline + shaped.descent;

    Some(DropCapPlan {
        n_lines,
        body_inset,
        items,
        bottom,
    })
}

/// A shaped cap: glyph items (baseline-relative, at x origin 0) plus metrics.
struct ShapedCap {
    items: Vec<PositionedItem>,
    advance: f32,
    ascent: f32,
    descent: f32,
}

/// Shapes `cap_text` at `font_size` using `base`'s family/weight/style/colour,
/// returning glyph runs whose origin is the baseline at `(0, 0)`.
fn shape_cap(
    resources: &mut FontResources,
    cap_text: &str,
    base: &StyleSpan,
    font_size: f32,
    display_scale: f32,
) -> Option<ShapedCap> {
    let mut builder =
        resources
            .layout_cx
            .ranged_builder(&mut resources.font_cx, cap_text, display_scale, true);
    builder.push_default(StyleProperty::Brush(base.color));
    builder.push_default(StyleProperty::FontSize(font_size));
    if base.weight != 400 {
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(
            base.weight as f32,
        )));
    }
    if base.italic {
        builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
    }
    if let Some(name) = &base.font_name {
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(name.as_str())));
    }
    let mut layout = builder.build(cap_text);
    layout.break_all_lines(None);
    layout.align(parley::Alignment::Start, AlignmentOptions::default());

    let line = layout.lines().next()?;
    let ascent = line.metrics().ascent;
    let descent = line.metrics().descent;

    let mut items = Vec::new();
    let mut advance = 0.0f32;
    for item in line.items() {
        let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
            continue;
        };
        let run = glyph_run.run();
        let run_offset = glyph_run.offset();
        let run_baseline = glyph_run.baseline();
        advance += glyph_run.advance();

        let raw: &[u8] = run.font().data.data();
        let font_data = resources
            .font_data_cache
            .entry(raw.as_ptr() as u64)
            .or_insert_with(|| std::sync::Arc::new(raw.to_vec()))
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
        // Baseline-relative: the run's baseline sits at local y = 0 and its
        // start at x = 0; `plan_drop_cap` then translates the whole cap to its
        // target `(cap_x, cap_baseline)`. (Glyph x/y are already made relative
        // to `run_offset`/`run_baseline` below.)
        items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
            origin: LayoutPoint { x: 0.0, y: 0.0 },
            font_data,
            font_index: run.font().index,
            font_size: run.font_size(),
            glyphs,
            color: base.color,
            synthesis: GlyphSynthesis {
                bold: synthesis.embolden(),
                italic: synthesis.skew().is_some(),
            },
            link_url: None,
        }));
    }
    if items.is_empty() {
        return None;
    }
    Some(ShapedCap {
        items,
        advance,
        ascent,
        descent,
    })
}

fn pts(p: loki_primitives::units::Points) -> f32 {
    p.value() as f32
}

#[cfg(test)]
#[path = "para_drop_cap_tests.rs"]
mod tests;
