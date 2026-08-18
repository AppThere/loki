// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spelling-squiggle emission and its per-run language routing.
//!
//! Split from `para_underlays.rs` when the I-06 fix pushed that file over the
//! 300-line ceiling. The cut is cohesive rather than arbitrary: everything here is
//! about *spelling*, and takes its input from a `SpellState`, while the parent
//! keeps the underlays driven purely by document markup (highlights, deletion
//! marks).

use parley::{Cursor, Layout, Selection};

use crate::color::LayoutColor;
use crate::items::{
    DecorationKind, DecorationStyle, FRAGMENT_CLIP_FLOOR_SLACK_PT, PositionedDecoration,
    PositionedItem,
};

use super::super::{ResolvedParaProps, StyleSpan};
use super::{SPELL_SQUIGGLE_COLOR, line_indent};

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
pub(crate) fn emit_spelling_squiggles(
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
    // Per-line box bottom — the boundary a split fragment's clip is placed on,
    // and therefore the only correct ceiling for the band.
    //
    // Not the same as the selection rect's `bb.y1`, which is what this used to
    // clamp against: `Selection::geometry` sizes its rect to the *font's* natural
    // height, so under an exact line height shorter than the font (I-06's case —
    // `Exact(12pt)` at 14pt) it overhangs the following line by the difference.
    // Clamping to it therefore permitted a band below the very boundary it was
    // meant to hold the band above.
    //
    // Clamped to the layout height as well, because the two bounds a clip can be
    // placed on are *different derivations of the same fact* and disagree by a
    // fraction of a point: a split fragment clips to the line's
    // `block_max_coord`, while the final fragment clips to `Layout::height()`,
    // and `flow_split` documents that the last line's max can sit either side of
    // it. Taking the lower of the two is what makes this hold for both.
    let layout_height = layout.height();
    let line_box_bottom: Vec<f32> = layout
        .lines()
        .map(|l| l.metrics().block_max_coord.min(layout_height))
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
                // Keep the whole band inside the region a split fragment's clip is
                // guaranteed to keep (I-06). The clip is placed on the **line box
                // bottom**, and the renderer floors it to a whole device pixel, so
                // the last line of a fragment can lose up to
                // `FRAGMENT_CLIP_FLOOR_SLACK_PT` of that edge — and with an exact
                // line height there is no leading between the descender and the
                // box bottom to absorb it, so the band crossed the boundary and
                // was cut on both sides.
                //
                // Only ever raises the band, and only when the tight case leaves it
                // no room, so ordinary leading is untouched and the rendering of
                // every document that is not using exact line heights is
                // unchanged — which is why this does not churn the goldens.
                let box_bottom = line_box_bottom
                    .get(line_idx)
                    .copied()
                    .unwrap_or(bb.y1 as f32);
                let latest_band_bottom = box_bottom - FRAGMENT_CLIP_FLOOR_SLACK_PT;
                let y = (descender - thickness / 2.0).min(latest_band_bottom - thickness);
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: bb.x0 as f32 + indent,
                    y,
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
