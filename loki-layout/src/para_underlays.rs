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
    DecorationKind, DecorationStyle, PositionedDecoration, PositionedItem, PositionedRect,
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

/// Emit a filled rect behind each highlighted span. `span.highlight_color`
/// already folds in the run-shading (`w:shd` / `fo:background-color`) fallback.
pub(super) fn emit_highlight_underlays(
    items: &mut Vec<PositionedItem>,
    layout: &Layout<LayoutColor>,
    clean_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    drop_lines: usize,
    drop_shift: f32,
) {
    for span in clean_spans {
        let Some(hl) = span.highlight_color else {
            continue;
        };
        if span.range.start >= span.range.end {
            continue;
        }
        let anchor =
            Cursor::from_byte_index(layout, span.range.start, parley::Affinity::Downstream);
        let focus = Cursor::from_byte_index(layout, span.range.end, parley::Affinity::Downstream);
        for (bb, line_idx) in Selection::new(anchor, focus).geometry(layout) {
            let indent = line_indent(para_props, line_idx, drop_lines, drop_shift);
            items.push(PositionedItem::FilledRect(PositionedRect {
                rect: LayoutRect::new(
                    bb.x0 as f32 + indent,
                    bb.y0 as f32,
                    (bb.x1 - bb.x0) as f32,
                    (bb.y1 - bb.y0) as f32,
                ),
                color: hl,
            }));
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
pub(super) fn emit_spelling_squiggles(
    items: &mut Vec<PositionedItem>,
    layout: &Layout<LayoutColor>,
    clean_text: &str,
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
    for miss in spell.checker.check_text(clean_text) {
        if miss.range.start >= miss.range.end {
            continue;
        }
        let anchor =
            Cursor::from_byte_index(layout, miss.range.start, parley::Affinity::Downstream);
        let focus = Cursor::from_byte_index(layout, miss.range.end, parley::Affinity::Downstream);
        for (bb, line_idx) in Selection::new(anchor, focus).geometry(layout) {
            let indent = line_indent(para_props, line_idx, drop_lines, drop_shift);
            let thickness = (((bb.y1 - bb.y0) as f32) * 0.06).clamp(0.7, 1.5);
            // Top of the squiggle band = the descender bottom, so the wave rides
            // just under the glyphs. Fall back to the line-box bottom if the line
            // index is somehow out of range.
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
