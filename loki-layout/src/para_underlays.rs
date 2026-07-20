// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Selection-geometry underlays for paragraph layout: highlight/background
//! fills (gap #10) and spelling squiggles. Split out of `para.rs` (Phase 7.1);
//! both are emitted *behind* the glyph runs by `layout_paragraph_uncached`.
//!
//! Each resolves a byte range to its exact per-line extents via Parley
//! selection geometry — robust to Parley coalescing adjacent runs that differ
//! only in an attribute Parley does not track (highlight colour) — and appends
//! renderer-agnostic items.

use parley::{Cursor, Layout, Selection};

use crate::color::LayoutColor;
use crate::geometry::LayoutRect;
use crate::items::{
    DecorationKind, DecorationStyle, PositionedBorderRect, PositionedDecoration, PositionedItem,
    PositionedRect,
};

use super::{ResolvedParaProps, StyleSpan};

/// Colour of the spelling squiggle (opaque red), matching the convention used
/// by desktop word processors for misspelled words.
const SPELL_SQUIGGLE_COLOR: LayoutColor = LayoutColor {
    r: 0.84,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// Left indent of a wrapped line: the hanging first line pulls left by the
/// hanging amount; leading lines beside a drop cap / float band shift right.
fn line_indent(
    para_props: &ResolvedParaProps,
    line_idx: usize,
    drop_lines: usize,
    drop_shift: f32,
) -> f32 {
    let mut indent = if line_idx == 0 && para_props.indent_hanging > 0.0 {
        para_props.indent_start - para_props.indent_hanging
    } else {
        para_props.indent_start
    };
    if line_idx < drop_lines {
        indent += drop_shift;
    }
    indent
}

/// Invoke `f` with each per-line layout rect of `span`'s byte range (Parley
/// selection geometry, indent-adjusted). Shared by the highlight and
/// character-border underlays.
fn for_span_line_rects(
    layout: &Layout<LayoutColor>,
    span: &StyleSpan,
    para_props: &ResolvedParaProps,
    drop_lines: usize,
    drop_shift: f32,
    mut f: impl FnMut(LayoutRect),
) {
    if span.range.start >= span.range.end {
        return;
    }
    let anchor = Cursor::from_byte_index(layout, span.range.start, parley::Affinity::Downstream);
    let focus = Cursor::from_byte_index(layout, span.range.end, parley::Affinity::Downstream);
    for (bb, line_idx) in Selection::new(anchor, focus).geometry(layout) {
        let indent = line_indent(para_props, line_idx, drop_lines, drop_shift);
        f(LayoutRect::new(
            bb.x0 as f32 + indent,
            bb.y0 as f32,
            (bb.x1 - bb.x0) as f32,
            (bb.y1 - bb.y0) as f32,
        ));
    }
}

/// Emit each run's background/border underlays: a filled rect behind a
/// highlighted span (`span.highlight_color` folds in the `w:shd` /
/// `fo:background-color` fallback) and a border box around a `w:bdr` character
/// border — both resolved per visual line via Parley selection geometry.
pub(super) fn emit_highlight_underlays(
    items: &mut Vec<PositionedItem>,
    layout: &Layout<LayoutColor>,
    clean_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    drop_lines: usize,
    drop_shift: f32,
) {
    for span in clean_spans {
        if let Some(hl) = span.highlight_color {
            for_span_line_rects(layout, span, para_props, drop_lines, drop_shift, |rect| {
                items.push(PositionedItem::FilledRect(PositionedRect {
                    rect,
                    color: hl,
                }));
            });
        }
        if let Some(edge) = span.character_border {
            for_span_line_rects(layout, span, para_props, drop_lines, drop_shift, |rect| {
                items.push(PositionedItem::BorderRect(PositionedBorderRect {
                    rect,
                    top: Some(edge),
                    right: Some(edge),
                    bottom: Some(edge),
                    left: Some(edge),
                }));
            });
        }
    }
}

/// Emit a struck, author-coloured end-of-paragraph marker when the paragraph's
/// ¶ carries a tracked deletion (Review tab, 4a.2 para-mark rendering).
///
/// Not a shaped ¶ glyph: two short stems + a strikethrough segment placed just
/// after the last line's text — paint-only items, so caret placement,
/// hit-testing, and text wrapping are untouched (the constraint that deferred
/// this). Accepting/rejecting the change clears the mark and the marker
/// disappears with the next layout.
pub(super) fn emit_para_mark_deletion(
    items: &mut Vec<PositionedItem>,
    layout: &Layout<LayoutColor>,
    para_props: &ResolvedParaProps,
    drop_lines: usize,
    drop_shift: f32,
) {
    let Some(color) = para_props.para_mark_deleted_color else {
        return;
    };
    let line_count = layout.lines().count();
    let Some(line) = layout.lines().last() else {
        return;
    };
    let m = line.metrics();
    let indent = line_indent(para_props, line_count - 1, drop_lines, drop_shift);
    // Marker box after the line's text: ~0.5 em wide, stems rising ~0.6 of the
    // ascent from the baseline, struck through the middle in the author colour.
    let em = (m.ascent + m.descent).max(1.0);
    let x0 = m.advance + indent + em * 0.15;
    let width = em * 0.5;
    let stem_h = m.ascent * 0.6;
    let stem_w = (em * 0.06).clamp(0.6, 1.2);
    for stem_x in [x0 + width * 0.25, x0 + width * 0.65] {
        items.push(PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(stem_x, m.baseline - stem_h, stem_w, stem_h),
            color,
        }));
    }
    items.push(PositionedItem::Decoration(PositionedDecoration {
        x: x0,
        // `y` is the TOP of the decoration band (see loki-vello `decor.rs`).
        y: m.baseline - stem_h * 0.5 - stem_w / 2.0,
        width,
        thickness: stem_w,
        kind: DecorationKind::Strikethrough,
        style: DecorationStyle::Solid,
        color,
    }));
}

/// Emit a `DecorationKind::Spelling` wave under each misspelled word the
/// supplied checker flags in `clean_text`. Thickness scales with line height;
/// the wave is anchored just below the text descender (the run underline zone),
/// so it hugs the glyphs instead of floating in the inter-line leading.
///
/// Per-run language routing (gap #30): the text is segmented by each span's
/// resolved checker ([`crate::SpellState::checker_for`]) and each segment is
/// checked with its own dictionary; segments whose language resolves to no
/// checker (multi-dictionary mode) are skipped, so foreign-language runs are
/// not blanketed in false squiggles.
// One arg over the limit; grouping (text, spans) or (drop_lines, drop_shift)
// into ad-hoc structs for a private emitter obscures more than it helps —
// same call as the flow_cell_blocks precedent.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_spelling_squiggles(
    items: &mut Vec<PositionedItem>,
    layout: &Layout<LayoutColor>,
    clean_text: &str,
    clean_spans: &[StyleSpan],
    spell: Option<&crate::SpellState>,
    para_props: &ResolvedParaProps,
    drop_lines: usize,
    drop_shift: f32,
) {
    let Some(spell) = spell else {
        return;
    };
    // Per-line descender bottom (`baseline + descent`) — where a run underline
    // sits, and the tight anchor for the squiggle. Selection geometry only gives
    // the full line box (`bb`), whose bottom includes leading below the glyphs.
    let line_descender_bottom: Vec<f32> = layout
        .lines()
        .map(|l| {
            let m = l.metrics();
            m.baseline + m.descent
        })
        .collect();
    for (seg, checker) in language_segments(clean_text, clean_spans, spell) {
        for miss in checker.check_text(&clean_text[seg.clone()]) {
            let (start, end) = (seg.start + miss.range.start, seg.start + miss.range.end);
            if start >= end {
                continue;
            }
            let anchor = Cursor::from_byte_index(layout, start, parley::Affinity::Downstream);
            let focus = Cursor::from_byte_index(layout, end, parley::Affinity::Downstream);
            for (bb, line_idx) in Selection::new(anchor, focus).geometry(layout) {
                let indent = line_indent(para_props, line_idx, drop_lines, drop_shift);
                let thickness = (((bb.y1 - bb.y0) as f32) * 0.06).clamp(0.7, 1.5);
                // Top of the squiggle band = the descender bottom, so the wave
                // rides just under the glyphs. Fall back to the line-box bottom
                // if the line index is somehow out of range.
                let descender = line_descender_bottom
                    .get(line_idx)
                    .copied()
                    .unwrap_or(bb.y1 as f32);
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: bb.x0 as f32 + indent,
                    y: descender - thickness / 2.0,
                    width: (bb.x1 - bb.x0) as f32,
                    thickness,
                    kind: DecorationKind::Spelling,
                    style: DecorationStyle::Wave,
                    color: SPELL_SQUIGGLE_COLOR,
                }));
            }
        }
    }
}

/// Split `text` into byte ranges each checked by one resolved checker.
///
/// The common case — no span carries a language tag — returns a single
/// whole-text segment on the default checker. Otherwise adjacent regions
/// resolving to the *same* checker are merged (so `en-US` next to `en-GB`
/// backed by one `en` dictionary stays a single segment and words crossing
/// the span boundary tokenize intact); regions resolving to no checker are
/// dropped. Text not covered by any span uses the default checker.
fn language_segments<'a>(
    text: &str,
    spans: &[StyleSpan],
    spell: &'a crate::SpellState,
) -> Vec<(
    std::ops::Range<usize>,
    &'a std::sync::Arc<loki_spell::SpellChecker>,
)> {
    if text.is_empty() {
        return Vec::new();
    }
    if spans.iter().all(|s| s.language.is_none()) {
        return vec![(0..text.len(), &spell.checker)];
    }
    type Seg<'s> = (
        std::ops::Range<usize>,
        Option<&'s std::sync::Arc<loki_spell::SpellChecker>>,
    );
    /// Append, merging into the previous segment when contiguous and resolved
    /// to the same checker (pointer identity).
    fn push<'s>(
        segs: &mut Vec<Seg<'s>>,
        range: std::ops::Range<usize>,
        checker: Option<&'s std::sync::Arc<loki_spell::SpellChecker>>,
    ) {
        if range.start >= range.end {
            return;
        }
        if let Some((last, last_checker)) = segs.last_mut() {
            let same = match (&*last_checker, &checker) {
                (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            if last.end == range.start && same {
                last.end = range.end;
                return;
            }
        }
        segs.push((range, checker));
    }
    let mut segs: Vec<Seg<'a>> = Vec::new();
    let mut cursor = 0usize;
    for span in spans {
        if span.range.start > cursor {
            push(&mut segs, cursor..span.range.start, Some(&spell.checker));
        }
        let start = span.range.start.max(cursor);
        if span.range.end > start {
            let checker = spell.checker_for(span.language.as_deref());
            push(&mut segs, start..span.range.end, checker);
            cursor = span.range.end;
        }
    }
    if cursor < text.len() {
        push(&mut segs, cursor..text.len(), Some(&spell.checker));
    }
    segs.into_iter()
        .filter_map(|(r, c)| c.map(|c| (r, c)))
        .collect()
}
