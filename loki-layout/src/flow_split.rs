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
        let page_remaining = state.page_content_height - state.cursor_y;

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
                state.cursor_y += resolved.space_before;
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
                break_column(state);
                frag_start = split_y;
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
                    state.cursor_y += resolved.space_before;
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
                break_column(state);
                frag_start = split_y;
                flushed_without_progress = false;
            }
        }
    }
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
    // Floor to prevent sub-pixel clip expansion.  Parley's max_coord equals
    // baseline + descent + leading_below; glyphs never reach max_coord, so
    // flooring by up to 1 pt never clips visible ink.  Without this, a
    // fractional max_coord × display-scale rounds up one physical pixel and
    // leaks the next line's top row through the clip. Fragment B uses unrounded
    // split_y for its translation (ty = -split_y), so the next page has no gap.
    let clip_height = (split_y - frag_start).floor();
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
    state.cursor_y += clip_height;
}
