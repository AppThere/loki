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

/// Emit a `DecorationKind::Spelling` wave under each misspelled word the
/// supplied checker flags in `clean_text`. Thickness scales with line height;
/// the wave is anchored near the line-box bottom.
///
/// TODO(spell-baseline): tighten to the run underline offset once verified
/// against the GPU renderer at multiple zooms (selection geometry yields the
/// line box, not per-run metrics).
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
            items.push(PositionedItem::Decoration(PositionedDecoration {
                x: bb.x0 as f32 + indent,
                y: bb.y1 as f32 - thickness * 2.5,
                width: (bb.x1 - bb.x0) as f32,
                thickness,
                kind: DecorationKind::Spelling,
                style: DecorationStyle::Wave,
                color: SPELL_SQUIGGLE_COLOR,
            }));
        }
    }
}
