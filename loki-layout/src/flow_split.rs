// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph page-splitting (split from `flow_para.rs` for the 300-line
//! ceiling): the core loop that walks a paragraph's Parley line boundaries,
//! emitting each page fragment as a clipped `PositionedItem::ClippedGroup`
//! (with widow/orphan control), and `emit_fragment`, the per-fragment
//! clip/translate emitter. `split_and_place_loop` is re-exported to the
//! `flow_para` parent, which calls it for both single paragraphs and chains.

use std::sync::Arc;

use super::widow_orphan;
use super::{FlowState, break_column, push_editing_para};
use crate::geometry::LayoutRect;
use crate::items::PositionedItem;
use crate::para::{ParagraphLayout, ResolvedParaProps};

/// Core splitting loop (ADR 004 §3).
///
/// Emits [`PositionedItem::ClippedGroup`] fragments when the paragraph spans
/// more than one page. Loops until all fragments are placed.
pub(super) fn split_and_place_loop(
    state: &mut FlowState,
    resolved: &ResolvedParaProps,
    para_layout: &ParagraphLayout,
    arc_layout: Option<Arc<ParagraphLayout>>,
    block_index: usize,
    dx: f32,
) {
    // paragraph-local y of the current fragment's top edge.
    let mut frag_start = 0.0f32;
    // Whether the current fragment has already triggered a page flush without
    // making progress. Guards against an infinite flush loop: with
    // `space_before > 0` a fresh page starts at `cursor_y == space_before` (> 0),
    // so the "flush and retry" arm below would otherwise fire every iteration
    // for a line taller than the page, pushing unbounded empty pages. After one
    // unproductive flush, the force-split arm runs instead.
    let mut flushed_without_progress = false;

    loop {
        let frag_height = para_layout.height - frag_start;

        // Nothing left to place. Reachable because the two quantities compared
        // here come from **different derivations of the same fact**:
        // `height` is Parley's `Layout::height()`, while `frag_start` is a line's
        // `block_max_coord` from `line_boundaries`, and the last line's max can
        // sit a fraction of a point *below* or *above* the layout height (with
        // Carlito at the default size: height 14.648, last boundary 15.0).
        // Without this guard the loop fell into the continuation arm with a
        // negative `frag_height`, emitting a `ClippedGroup` of negative height
        // that still carried the paragraph's glyph runs — the paragraph was
        // painted a second time, at a negative y, and `cursor_y` was moved
        // backwards for whatever followed. Caught by the two-column balancing
        // test once its font was pinned; before that the ambient face happened to
        // divide the column evenly and never landed here.
        if frag_height <= 0.0 {
            return;
        }
        // Break against the footnote-reserved content limit so lines stop above
        // this page's footnote band instead of overlapping it.
        let page_remaining = state.content_bottom() - state.cursor_y;

        if frag_height <= page_remaining {
            // Remaining fragment fits on the current page.
            let ty = state.cursor_y - frag_start;
            if frag_start < f32::EPSILON {
                // First (and only) fragment: emit items directly without clip.
                if let Some(ref al) = arc_layout {
                    push_editing_para(state, block_index, al.clone(), (0.0, ty));
                }
                for item in &para_layout.items {
                    let mut item = item.clone();
                    item.translate(dx, ty);
                    state.current_items.push(item);
                }
            } else {
                // Continuation fragment: clip to hide content from prior pages,
                // carrying only the items near its y-range (Option B, 6.3).
                if let Some(ref al) = arc_layout {
                    push_editing_para(state, block_index, al.clone(), (0.0, ty));
                }
                let clip_rect =
                    LayoutRect::new(0.0, state.cursor_y, state.content_width, frag_height);
                let mut items = para_layout.items_in_y_range(frag_start, para_layout.height);
                for item in &mut items {
                    item.translate(dx, ty);
                }
                state
                    .current_items
                    .push(PositionedItem::ClippedGroup { clip_rect, items });
            }
            super::super::line_numbers::emit(
                state,
                para_layout,
                ty,
                frag_start,
                para_layout.height,
            );
            state.cursor_y += frag_height;
            return;
        }

        // Find split_k: largest k such that line_boundaries[k].1 ≤ frag_start + page_remaining.
        // The boundary must also lie strictly past frag_start, otherwise the
        // split makes no progress (zero-height fragment → infinite loop).
        let max_visible_y = frag_start + page_remaining;
        let split_k = (0..para_layout.line_boundaries.len()).rev().find(|&k| {
            let line_max = para_layout.line_boundaries[k].1;
            line_max > frag_start && line_max <= max_visible_y
        });

        match split_k {
            None if state.cursor_y > 0.0 && !flushed_without_progress => {
                // No lines of this fragment fit in the current column; advance to
                // the next column (or page) and retry. Re-apply space_before on
                // the fresh column (ADR 004 §3 retry).
                break_column(state);
                state.advance_space_before(resolved.space_before);
                flushed_without_progress = true;
            }
            None => {
                // Even a full fresh page cannot fit a single line of this fragment
                // (a single line taller than the entire page height — extremely rare).
                // Force-split at the first line boundary past frag_start to
                // avoid an infinite loop; that line overflows its page and is
                // clipped, but layout terminates with bounded output.
                let split_y = para_layout
                    .line_boundaries
                    .iter()
                    .map(|&(_, max)| max)
                    .find(|&max| max > frag_start)
                    .unwrap_or(para_layout.height);
                if split_y <= frag_start {
                    // Still no progress: emit remainder and bail.
                    let ty = state.cursor_y - frag_start;
                    if let Some(ref al) = arc_layout {
                        push_editing_para(state, block_index, al.clone(), (0.0, ty));
                    }
                    for item in &para_layout.items {
                        let mut item = item.clone();
                        item.translate(dx, ty);
                        state.current_items.push(item);
                    }
                    state.cursor_y += frag_height;
                    return;
                }
                emit_fragment(
                    state,
                    para_layout,
                    arc_layout.clone(),
                    block_index,
                    frag_start,
                    split_y,
                    dx,
                );
                frag_start = split_y;
                if !break_for_remainder(state, para_layout, frag_start) {
                    return;
                }
                flushed_without_progress = false;
            }
            Some(k) => {
                // Widow/orphan control: `None` = defer the whole paragraph
                // (orphan); `Some(k')` = split there (a widow pulls `k'` back).
                let split_line = widow_orphan::resolve_split(
                    &para_layout.line_boundaries,
                    frag_start,
                    k,
                    usize::from(resolved.orphan_min),
                    usize::from(resolved.widow_min),
                    state.cursor_y > 0.0,
                );
                if split_line.is_none() && !flushed_without_progress {
                    // Orphan: move the whole paragraph to the next page (mirrors
                    // the "no lines fit" flush; guarded so the retry at the fresh
                    // page top splits normally and terminates).
                    break_column(state);
                    state.advance_space_before(resolved.space_before);
                    flushed_without_progress = true;
                    continue;
                }
                // Emit Fragment A covering para-local [frag_start, split_y). An
                // already-flushed orphan falls back to the natural split `k`.
                let split_y = para_layout.line_boundaries[split_line.unwrap_or(k)].1;
                emit_fragment(
                    state,
                    para_layout,
                    arc_layout.clone(),
                    block_index,
                    frag_start,
                    split_y,
                    dx,
                );
                frag_start = split_y;
                if !break_for_remainder(state, para_layout, frag_start) {
                    return;
                }
                flushed_without_progress = false;
            }
        }
    }
}

/// Open the next column/page for the fragment starting at `frag_start`, and
/// report whether there is one. `false` means the paragraph is fully placed and
/// the caller should stop **without** breaking.
///
/// Splitting always ended a fragment by breaking and letting the next iteration
/// notice it had nothing left. That left a column or page open for a fragment
/// that does not exist, which the final `finish_page` then emits as a blank one.
/// Whether it happened at all turned on whether the last line's
/// `block_max_coord` sat above `Layout::height()` — a fraction of a point that
/// pixel-quantised metrics happened to hide.
fn break_for_remainder(
    state: &mut FlowState,
    para_layout: &ParagraphLayout,
    frag_start: f32,
) -> bool {
    if para_layout.height - frag_start <= 0.0 {
        return false;
    }
    break_column(state);
    true
}

/// Emit a [`PositionedItem::ClippedGroup`] covering para-local y ∈ `[frag_start, split_y)`;
/// items translate so `frag_start` maps to `state.cursor_y`, which advances by
/// the fragment height.
fn emit_fragment(
    state: &mut FlowState,
    para_layout: &ParagraphLayout,
    arc_layout: Option<Arc<ParagraphLayout>>,
    block_index: usize,
    frag_start: f32,
    split_y: f32,
    dx: f32,
) {
    // The exact line boundary, deliberately *not* rounded here.
    //
    // The hazard this used to guard against is real but belongs a layer down: a
    // fractional clip height times the display scale can round up one physical
    // pixel and leak the next line's top row. That is a device-pixel effect, and
    // layout has no device pixels — it works in points and is painted at an
    // arbitrary zoom × DPI that it does not know. Flooring *here* therefore both
    // over-shaved (a whole point, up to 2 device pixels at 144 dpi) and could
    // not actually promise anything about the paint grid.
    //
    // The floor now lives in each renderer, which knows its own scale and floors
    // the clip's bottom edge to a whole device pixel — see
    // [`FRAGMENT_CLIP_FLOOR_SLACK_PT`](crate::items::FRAGMENT_CLIP_FLOOR_SLACK_PT).
    // Keeping the boundary exact here is what lets Parley run unquantized:
    // `split_y - frag_start` is then fractional, and flooring it in points cost
    // decoration placement its entire reserve.
    let clip_height = split_y - frag_start;
    let clip_rect = LayoutRect::new(0.0, state.cursor_y, state.content_width, clip_height);
    let ty = state.cursor_y - frag_start;
    if let Some(al) = arc_layout {
        push_editing_para(state, block_index, al, (0.0, ty));
    }
    // Option B (6.3): only the items near this fragment's y-range travel with
    // it; the clip masks the conservative slop, so rendering is unchanged.
    let mut items = para_layout.items_in_y_range(frag_start, split_y);
    for item in &mut items {
        item.translate(dx, ty);
    }
    state
        .current_items
        .push(PositionedItem::ClippedGroup { clip_rect, items });
    super::super::line_numbers::emit(state, para_layout, ty, frag_start, split_y);
    state.cursor_y += clip_height;
}
