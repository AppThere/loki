// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph placement with keep-together, line-boundary splitting, and
//! fragment emission (ADR 004 §3).

use std::sync::Arc;

use crate::geometry::LayoutRect;
use crate::items::PositionedItem;
use crate::para::{ParagraphLayout, ResolvedParaProps};
use crate::result::PageParagraphData;

use crate::flow::{FlowState, LayoutWarning};
use crate::flow_block::finish_page;

/// Place a pre-computed paragraph layout, handling `keep_together` and splitting.
///
/// `space_before` must already be reflected in `state.cursor_y` by the caller.
///
/// Non-fatal issues are pushed onto `state.warnings` rather than returned.
pub(super) fn place_paragraph_layout(
    state: &mut FlowState,
    resolved: &ResolvedParaProps,
    para_layout: ParagraphLayout,
    block_index: usize,
) {
    if !state.mode.is_paginated() {
        let dy = state.cursor_y;
        let dx = state.current_indent;
        if state.options.preserve_for_editing {
            state.current_paragraphs.push(PageParagraphData {
                block_index,
                layout: Arc::new(para_layout.clone()),
                origin: (0.0, dy),
            });
        }
        for mut item in para_layout.items {
            item.translate(dx, dy);
            state.current_items.push(item);
        }
        state.cursor_y += para_layout.height + resolved.space_after;
        return;
    }

    // keep-together: attempt to place all lines on one page (ADR 004 §4).
    if resolved.keep_together {
        let needed = para_layout.height + resolved.space_after;
        if needed > state.page_content_height {
            // Block exceeds full page height; cannot keep together anywhere.
            state.warnings.push(LayoutWarning::KeepTogetherOverride {
                block_index,
                block_height: para_layout.height,
            });
            tracing::warn!(
                block_index,
                block_height = para_layout.height,
                "keep-together paragraph exceeds page height; splitting"
            );
            // Fall through to normal splitting.
        } else {
            let available = state.page_content_height - state.cursor_y;
            if needed > available && state.cursor_y > 0.0 {
                finish_page(state);
                state.cursor_y += resolved.space_before;
            }
            // Fits on current (or freshly flushed) page.
            let dy = state.cursor_y;
            let dx = state.current_indent;
            if state.options.preserve_for_editing {
                state.current_paragraphs.push(PageParagraphData {
                    block_index,
                    layout: Arc::new(para_layout.clone()),
                    origin: (0.0, dy),
                });
            }
            for mut item in para_layout.items {
                item.translate(dx, dy);
                state.current_items.push(item);
            }
            state.cursor_y += para_layout.height + resolved.space_after;
            return;
        }
    }

    // Normal paginated placement with line-boundary splitting.
    if para_layout.height > state.page_content_height {
        state.warnings.push(LayoutWarning::BlockExceedsPageHeight {
            block_index,
            block_height: para_layout.height,
        });
    }

    let dx = state.current_indent;
    let arc_layout = if state.options.preserve_for_editing {
        Some(Arc::new(para_layout.clone()))
    } else {
        None
    };
    split_and_place_loop(state, resolved, &para_layout, arc_layout, block_index, dx);
    state.cursor_y += resolved.space_after;
}

/// Core splitting loop (ADR 004 §3).
///
/// Emits [`PositionedItem::ClippedGroup`] fragments when the paragraph spans
/// more than one page. Loops until all fragments are placed.
fn split_and_place_loop(
    state: &mut FlowState,
    resolved: &ResolvedParaProps,
    para_layout: &ParagraphLayout,
    arc_layout: Option<Arc<ParagraphLayout>>,
    block_index: usize,
    dx: f32,
) {
    // paragraph-local y of the current fragment's top edge.
    let mut frag_start = 0.0f32;

    loop {
        let frag_height = para_layout.height - frag_start;
        let page_remaining = state.page_content_height - state.cursor_y;

        if frag_height <= page_remaining {
            // Remaining fragment fits on the current page.
            let ty = state.cursor_y - frag_start;
            if frag_start < f32::EPSILON {
                // First (and only) fragment: emit items directly without clip.
                if let Some(ref al) = arc_layout {
                    state.current_paragraphs.push(PageParagraphData {
                        block_index,
                        layout: al.clone(),
                        origin: (0.0, ty),
                    });
                }
                for item in &para_layout.items {
                    let mut item = item.clone();
                    item.translate(dx, ty);
                    state.current_items.push(item);
                }
            } else {
                // Continuation fragment: clip to hide content from prior pages.
                if let Some(ref al) = arc_layout {
                    state.current_paragraphs.push(PageParagraphData {
                        block_index,
                        layout: al.clone(),
                        origin: (0.0, ty),
                    });
                }
                let clip_rect =
                    LayoutRect::new(0.0, state.cursor_y, state.content_width, frag_height);
                let mut items = para_layout.items.clone();
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
        let max_visible_y = frag_start + page_remaining;
        let split_k = (0..para_layout.line_boundaries.len())
            .rev()
            .find(|&k| para_layout.line_boundaries[k].1 <= max_visible_y);

        match split_k {
            None if state.cursor_y > 0.0 => {
                // No lines of this fragment fit on the current page; flush and retry.
                // Re-apply space_before on the fresh page (ADR 004 §3 retry).
                finish_page(state);
                state.cursor_y += resolved.space_before;
            }
            None => {
                // Even a full fresh page cannot fit a single line of this fragment
                // (a single line taller than the entire page height — extremely rare).
                // Force-split at line 0 to avoid an infinite loop.
                let split_y = para_layout
                    .line_boundaries
                    .first()
                    .map(|&(_, max)| max.max(frag_start + 1.0))
                    .unwrap_or(para_layout.height);
                if split_y <= frag_start {
                    // Still no progress: emit remainder and bail.
                    let ty = state.cursor_y - frag_start;
                    if let Some(ref al) = arc_layout {
                        state.current_paragraphs.push(PageParagraphData {
                            block_index,
                            layout: al.clone(),
                            origin: (0.0, ty),
                        });
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
                finish_page(state);
                frag_start = split_y;
            }
            Some(k) => {
                // Emit Fragment A covering para-local [frag_start, split_y).
                let split_y = para_layout.line_boundaries[k].1;
                emit_fragment(
                    state,
                    para_layout,
                    arc_layout.clone(),
                    block_index,
                    frag_start,
                    split_y,
                    dx,
                );
                finish_page(state);
                frag_start = split_y;
                // space_before is NOT re-applied between split fragments; only
                // the "no lines fit → flush" branch above re-applies it.
            }
        }
    }
}

/// Emit a [`PositionedItem::ClippedGroup`] covering para-local y ∈ `[frag_start, split_y)`.
///
/// Items are translated so para-local `frag_start` maps to page `state.cursor_y`.
/// Advances `state.cursor_y` by the fragment height.
pub(super) fn emit_fragment(
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
    // leaks the top row of the next line through the clip.
    // Fragment B uses unrounded split_y for its translation (ty = -split_y)
    // so there is no corresponding gap at the top of the next page.
    let clip_height = (split_y - frag_start).floor();
    let clip_rect = LayoutRect::new(0.0, state.cursor_y, state.content_width, clip_height);
    let ty = state.cursor_y - frag_start;
    if let Some(al) = arc_layout {
        state.current_paragraphs.push(PageParagraphData {
            block_index,
            layout: al,
            origin: (0.0, ty),
        });
    }
    let mut items = para_layout.items.clone();
    for item in &mut items {
        item.translate(dx, ty);
    }
    state
        .current_items
        .push(PositionedItem::ClippedGroup { clip_rect, items });
    state.cursor_y += clip_height;
}
