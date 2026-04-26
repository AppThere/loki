// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Paragraph placement, splitting, and keep-with-next chain logic.
//!
//! Split algorithm (ADR 004 §3): when a paragraph does not fit on the current
//! page, it is split at the last Parley line boundary that fits. Each fragment
//! is wrapped in a [`PositionedItem::ClippedGroup`] so that full-height
//! background and border items are clipped correctly.
//!
//! # Session 3 pre-audit findings (2026-04-20)
//!
//! ## Q1 — indent_hanging coverage (55f489b)
//! `indent_hanging` is applied in `layout_paragraph` for ALL paragraphs
//! (not just list items): the glyph-run loop shifts line 0 left by
//! `indent_hanging` unconditionally when the field is > 0. Non-list paragraphs
//! with a manually set `indent_hanging` therefore produce the correct first-line
//! offset. One known gap: `line_w` passed to `break_all_lines` is computed as
//! `available_width − indent_start − indent_end` for every line, so Parley
//! wraps line 0 at the same column as continuation lines. The first line
//! physically starts `indent_hanging` to the left but wraps `indent_hanging` too
//! early. Fixing this requires per-line width, which Parley 0.6 does not expose;
//! the inaccuracy is minor (≤ one word per line) and non-blocking for Session 3.
//!
//! ## Q2 — Parley 0.6 bidi API
//! `BidiLevel` and `BidiResolver` are `pub(crate)` — no public API exists to
//! set a per-paragraph base direction. There is no `StyleProperty` variant for
//! text direction in Parley 0.6's `StyleProperty` enum
//! (`FontStack`, `FontSize`, `FontStyle`, `FontWeight`, `Underline`,
//! `Strikethrough`, `LineHeight`, `WordSpacing`, `LetterSpacing`, `WordBreak`,
//! `OverflowWrap`, `Locale` — no RTL/bidi entry). Parley runs the Unicode BiDi
//! algorithm automatically on character class properties. Gap #5 (RTL paragraph
//! direction) cannot be addressed via `StyleProperty`; the only workaround is
//! embedding Unicode directional control characters (U+202B RLE / U+200F RLM)
//! into the text string. Defer to a future Parley version or a separate session.
//!
//! ## Q3 — page_break_after hook point
//! `page_break_after` is absent from `ResolvedParaProps` (only `page_break_before`
//! is present). The natural hook is in `flow_paragraph` (this file, currently
//! line 95) immediately after `place_paragraph_layout(state, &resolved, …)`.
//! Adding it is a 4-line change: add `page_break_after: bool` to
//! `ResolvedParaProps` (para.rs), forward from `ParaProps` in `map_para_props`
//! (resolve.rs), and add after `place_paragraph_layout`:
//! ```text
//! if resolved.page_break_after && state.mode.is_paginated() {
//!     finish_page(state);
//! }
//! ```

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::list_style::ListLevelKind;

use crate::geometry::LayoutRect;
use crate::items::{PositionedImage, PositionedItem};
use crate::para::{format_list_marker, layout_paragraph, ParagraphLayout, ResolvedParaProps};
use crate::resolve::{emu_to_pt, flatten_paragraph, resolve_para_props};

use super::{finish_page, FlowState, LayoutWarning};

/// Maximum keep-with-next chain length before truncation (ADR 004 §4).
const CHAIN_LIMIT: usize = 5;

// ── Public(super) API ─────────────────────────────────────────────────────────

/// Resolve, lay out, and place a single paragraph block.
pub(super) fn flow_paragraph(
    state: &mut FlowState,
    para: &StyledParagraph,
    block_index: usize,
) {
    let resolved = resolve_para_props(para, state.catalog);

    // ── List marker synthesis ────────────────────────────────────────────────
    // When the paragraph carries list membership, look up the list style,
    // advance the per-list counter, format the marker string, and prepend it
    // as an `Inline::Str` followed by a tab. Non-list paragraphs reset
    // `prev_list_id` so the next list starts fresh.
    let owned_para: Option<StyledParagraph> = if let Some(ref lm) = resolved.list_marker {
        if let Some(list_style) = state.catalog.list_styles.get(&lm.list_id) {
            if let Some(level_def) = list_style.levels.get(lm.level as usize) {
                let start_value = match &level_def.kind {
                    ListLevelKind::Numbered { start_value, .. } => *start_value,
                    _ => 1,
                };
                // New-list detection: a different list_id means a new list
                // is starting, so counters for this id are cleared.
                if state.prev_list_id.as_ref() != Some(&lm.list_id) {
                    state.list_counters.remove(&lm.list_id);
                }
                state.prev_list_id = Some(lm.list_id.clone());
                state.advance_counter(&lm.list_id, lm.level, start_value);
                let counters = state.list_counters
                    .get(&lm.list_id)
                    .copied()
                    .unwrap_or([1u32; 9]);
                let marker_text =
                    format_list_marker(&list_style.levels, lm.level, &counters);
                let mut cloned = para.clone();
                cloned.inlines.insert(0, Inline::Str(format!("{}\t", marker_text).into()));
                Some(cloned)
            } else {
                state.prev_list_id = None;
                None
            }
        } else {
            state.prev_list_id = None;
            None
        }
    } else {
        state.prev_list_id = None;
        None
    };
    let effective_para: &StyledParagraph = owned_para.as_ref().unwrap_or(para);
    // ────────────────────────────────────────────────────────────────────────

    let (text, spans, images, notes) = flatten_paragraph(effective_para, state.catalog, &mut state.note_counter);
    state.pending_footnotes.extend(notes);

    state.cursor_y += resolved.space_before;

    if resolved.page_break_before && state.mode.is_paginated() {
        finish_page(state);
    }

    let mut para_layout = layout_paragraph(
        state.resources,
        &text,
        &spans,
        &resolved,
        state.content_width,
        state.display_scale,
        state.options.preserve_for_editing,
    );

    // ── Inline image placement (gap #9) ──────────────────────────────────────
    // TODO(inline-image-flow): Parley has no inline image box support.
    // Images are prepended as a block-level prefix before paragraph text;
    // all existing items are shifted down to make room.
    let mut total_image_height = 0.0f32;
    let mut image_items: Vec<PositionedItem> = Vec::new();
    for img in &images {
        if img.cx_emu == 0 && img.cy_emu == 0 {
            continue; // zero-size image — skip without crashing
        }
        let w = emu_to_pt(img.cx_emu);
        let h = emu_to_pt(img.cy_emu);
        image_items.push(PositionedItem::Image(PositionedImage {
            rect: LayoutRect::new(0.0, total_image_height, w, h),
            src: img.src.clone(),
            alt: img.alt.clone(),
        }));
        total_image_height += h;
    }
    if total_image_height > 0.0 {
        // Expand background fill to cover image area (first item when present).
        if let Some(PositionedItem::FilledRect(bg)) = para_layout.items.first_mut() {
            bg.rect.size.height += total_image_height;
        }
        // Shift all existing paragraph items down by total image height.
        for item in &mut para_layout.items {
            item.translate(0.0, total_image_height);
        }
        para_layout.height += total_image_height;
        // Prepend image items (they render before paragraph text).
        image_items.extend(para_layout.items.drain(..));
        para_layout.items = image_items;
    }

    place_paragraph_layout(state, &resolved, para_layout, block_index);

    if resolved.page_break_after && state.mode.is_paginated() {
        finish_page(state);
    }
}

/// Place a pre-computed paragraph layout, handling `keep_together` and splitting.
///
/// `space_before` must already be reflected in `state.cursor_y` by the caller.
///
/// # Errors
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
    split_and_place_loop(state, resolved, &para_layout, dx);
    state.cursor_y += resolved.space_after;
}

/// Handle a `keep_with_next` chain of top-level section blocks.
///
/// Scans forward from `start`, speculatively lays out all chain blocks, then
/// decides whether to flush the current page before placing the chain.
///
/// Returns the number of section blocks consumed so the caller can skip them.
pub(super) fn flow_keep_with_next_chain(
    state: &mut FlowState,
    blocks: &[Block],
    start: usize,
) -> usize {
    // Scan: each block with keep_with_next=true "pulls" the block after it.
    // chain_end is the index of the last block included in the chain.
    let mut chain_end = start;
    let mut natural_len = 1usize;

    loop {
        let has_kwn = if let Block::StyledPara(p) = &blocks[chain_end] {
            resolve_para_props(p, state.catalog).keep_with_next
        } else {
            false
        };
        if !has_kwn || chain_end + 1 >= blocks.len() {
            break;
        }
        natural_len += 1;
        chain_end += 1;
    }

    if natural_len > CHAIN_LIMIT {
        chain_end = start + CHAIN_LIMIT - 1;
        state.warnings.push(LayoutWarning::KeepWithNextChainTruncated {
            start_block: start,
            chain_length: natural_len,
        });
        tracing::warn!(start_block = start, "keep-with-next chain exceeds 5; truncating");
    }

    // Speculatively layout all chain blocks to measure total height.
    let chain = build_chain_layouts(state, blocks, start, chain_end);
    let total_h: f32 = chain.iter()
        .map(|(r, l)| r.space_before + l.height + r.space_after)
        .sum();
    let chain_len = chain_end - start + 1;

    if total_h > state.page_content_height {
        return place_chain_too_tall(state, chain, start, chain_end, total_h);
    }

    let available = state.page_content_height - state.cursor_y;
    if total_h > available && state.cursor_y > 0.0 {
        finish_page(state);
    }

    place_chain_blocks(state, chain, start);
    chain_len
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Speculatively lay out blocks `start..=end` and return `(resolved, layout)` pairs.
fn build_chain_layouts<'s>(
    state: &mut FlowState<'s>,
    blocks: &[Block],
    start: usize,
    end: usize,
) -> Vec<(ResolvedParaProps, ParagraphLayout)> {
    (start..=end)
        .map(|idx| {
            if let Block::StyledPara(para) = &blocks[idx] {
                let resolved = resolve_para_props(para, state.catalog);
                let mut temp_counter = state.note_counter;
                let (text, spans, _images, _notes) = flatten_paragraph(para, state.catalog, &mut temp_counter);
                let layout = layout_paragraph(
                    state.resources,
                    &text,
                    &spans,
                    &resolved,
                    state.content_width,
                    state.display_scale,
                    state.options.preserve_for_editing,
                );
                (resolved, layout)
            } else {
                // Non-para block: contribute zero height in chain context.
                (ResolvedParaProps::default(), ParagraphLayout {
                    height: 0.0,
                    width: 0.0,
                    items: vec![],
                    first_baseline: 0.0,
                    last_baseline: 0.0,
                    line_boundaries: vec![],
                    parley_layout: None,
                })
            }
        })
        .collect()
}

/// Place chain blocks in order, adding `space_before` to `cursor_y` before each.
fn place_chain_blocks(
    state: &mut FlowState,
    chain: Vec<(ResolvedParaProps, ParagraphLayout)>,
    start: usize,
) {
    for (i, (resolved, layout)) in chain.into_iter().enumerate() {
        state.cursor_y += resolved.space_before;
        if resolved.page_break_before && state.mode.is_paginated() {
            finish_page(state);
        }
        place_paragraph_layout(state, &resolved, layout, start + i);
    }
}

/// Handle a chain that is taller than one page: find the prefix that fits,
/// emit `KeepWithNextChainTooTall`, flush if needed, place the prefix.
///
/// Returns the number of blocks consumed (the fitting prefix only; remaining
/// blocks fall back to the caller's main loop).
fn place_chain_too_tall(
    state: &mut FlowState,
    chain: Vec<(ResolvedParaProps, ParagraphLayout)>,
    start: usize,
    chain_end: usize,
    _total_h: f32,
) -> usize {
    // Find largest prefix whose total height fits on one fresh page.
    let mut prefix_h = 0.0f32;
    let mut last_fits = start;
    for (i, (resolved, layout)) in chain.iter().enumerate() {
        let block_h = resolved.space_before + layout.height + resolved.space_after;
        if prefix_h + block_h > state.page_content_height {
            break;
        }
        prefix_h += block_h;
        last_fits = start + i;
    }
    let break_at = last_fits + 1;

    state.warnings.push(LayoutWarning::KeepWithNextChainTooTall { start_block: start, break_at });
    tracing::warn!(
        start_block = start,
        end_block = chain_end,
        "keep-with-next chain too tall for one page; breaking at block {break_at}"
    );

    if state.cursor_y > 0.0 {
        finish_page(state);
    }

    let consumed = last_fits - start + 1;
    for (i, (resolved, layout)) in chain.into_iter().enumerate() {
        if start + i > last_fits {
            break;
        }
        state.cursor_y += resolved.space_before;
        if resolved.page_break_before && state.mode.is_paginated() {
            finish_page(state);
        }
        place_paragraph_layout(state, &resolved, layout, start + i);
    }
    consumed
}

/// Core splitting loop (ADR 004 §3).
///
/// Emits [`PositionedItem::ClippedGroup`] fragments when the paragraph spans
/// more than one page. Loops until all fragments are placed.
fn split_and_place_loop(
    state: &mut FlowState,
    resolved: &ResolvedParaProps,
    para_layout: &ParagraphLayout,
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
                for item in &para_layout.items {
                    let mut item = item.clone();
                    item.translate(dx, ty);
                    state.current_items.push(item);
                }
            } else {
                // Continuation fragment: clip to hide content from prior pages.
                let clip_rect = LayoutRect::new(
                    0.0,
                    state.cursor_y,
                    state.content_width,
                    frag_height,
                );
                let mut items = para_layout.items.clone();
                for item in &mut items {
                    item.translate(dx, ty);
                }
                state.current_items.push(PositionedItem::ClippedGroup { clip_rect, items });
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
                    for item in &para_layout.items {
                        let mut item = item.clone();
                        item.translate(dx, ty);
                        state.current_items.push(item);
                    }
                    state.cursor_y += frag_height;
                    return;
                }
                emit_fragment(state, para_layout, frag_start, split_y, dx);
                finish_page(state);
                frag_start = split_y;
            }
            Some(k) => {
                // Emit Fragment A covering para-local [frag_start, split_y).
                let split_y = para_layout.line_boundaries[k].1;
                emit_fragment(state, para_layout, frag_start, split_y, dx);
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
fn emit_fragment(
    state: &mut FlowState,
    para_layout: &ParagraphLayout,
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
    let mut items = para_layout.items.clone();
    for item in &mut items {
        item.translate(dx, ty);
    }
    state.current_items.push(PositionedItem::ClippedGroup { clip_rect, items });
    state.cursor_y += clip_height;
}
