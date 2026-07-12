// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph placement, splitting, and keep-with-next chain logic.
//!
//! Split algorithm (ADR 004 §3): a paragraph that does not fit is split at
//! the last fitting Parley line boundary; each fragment is wrapped in a
//! [`PositionedItem::ClippedGroup`] so full-height background/border items
//! clip correctly. `indent_hanging` shifts line 0 left for all paragraphs.
//! Known minor gap: `line_w` for `break_all_lines` is uniform across lines,
//! so line 0 wraps `indent_hanging` too early (no per-line width in Parley).
//!
//! Parley bidi note (gap #5): `BidiLevel`/`BidiResolver` are `pub(crate)` and
//! no `StyleProperty` sets a per-paragraph base direction — RTL direction is
//! deferred to a future Parley (workaround would be U+202B/U+200F controls).

use std::sync::Arc;

use loki_doc_model::content::block::{Block, StyledParagraph};

use crate::geometry::LayoutRect;
use crate::items::{PositionedImage, PositionedItem};
use crate::para::{ParagraphLayout, ResolvedParaProps, layout_paragraph_spelled};
use crate::resolve::{emu_to_pt, pts_to_f32, resolve_para_props};

use super::columns_impl::break_column;
use super::editing::push_editing_para;
use super::{FlowState, LayoutWarning, finish_page};

#[path = "flow_widow_orphan.rs"]
mod widow_orphan;

/// Maximum keep-with-next chain length before truncation (ADR 004 §4).
const CHAIN_LIMIT: usize = 5;

// ── Public(super) API ─────────────────────────────────────────────────────────

/// Resolve, lay out, and place a single paragraph block.
pub(super) fn flow_paragraph(state: &mut FlowState, para: &StyledParagraph, block_index: usize) {
    let mut resolved = resolve_para_props(para, state.catalog);
    // Between-border group adjustment (gap #26), staged by the block loop.
    if let Some(ovr) = state.staged_between.take() {
        if ovr.suppress_top {
            resolved.border_top = None;
        }
        if let Some(bottom) = ovr.bottom {
            resolved.border_bottom = bottom;
        }
    }
    // Cell-content word-breaking: long unbreakable words wrap to the column.
    resolved.break_long_words = state.break_long_words;
    // Document default tab-stop interval (Word `w:defaultTabStop`), when set.
    if let Some(pt) = state.options.default_tab_stop_pt {
        resolved.default_tab_stop = pt;
    }

    // ── List level indentation fallback ─────────────────────────────────────
    // The numbering level's pPr is the authoritative indent when the paragraph
    // carries none (both indents 0.0) — e.g. `w:ind` only on the abstract num.
    if let Some(ref lm) = resolved.list_marker
        && resolved.indent_start == 0.0
        && resolved.indent_hanging == 0.0
        && let Some(list_style) = state.catalog.list_styles.get(&lm.list_id)
        && let Some(level_def) = list_style.levels.get(lm.level as usize)
    {
        let level_indent = pts_to_f32(level_def.indent_start);
        let level_hanging = pts_to_f32(level_def.hanging_indent);
        if level_indent > 0.0 || level_hanging > 0.0 {
            resolved.indent_start = level_indent;
            resolved.indent_hanging = level_hanging;
        }
    }

    // ── List marker synthesis ────────────────────────────────────────────────
    // Prepend the label (bullet / number) as an `Inline::Str` + tab; a picture
    // bullet instead reports its image `src` for out-of-band placement below.
    let marker = super::flow_list_marker::synthesize(state, para, &resolved);
    let effective_para: &StyledParagraph = marker.owned.as_ref().unwrap_or(para);
    // ────────────────────────────────────────────────────────────────────────

    let (text, spans, mut images, mut notes) = crate::resolve::flatten_paragraph_with_base(
        effective_para,
        state.catalog,
        &mut state.note_counter,
        state.cell_char_defaults.as_ref(),
    );
    // Tag each note with its owning block + per-block order (see flow_footnotes).
    for (i, note) in notes.iter_mut().enumerate() {
        note.owner_block_index = block_index;
        note.note_in_block = i;
    }
    state.pending_footnotes.extend(notes);

    // ── Floating image wrap (gap #12): reserve a side band so text wraps
    // beside the float (emitted after text layout; removed from the
    // inline/block set so it is not also stacked above the text).
    let float_plan = super::float_impl::plan_float(&images, state.content_width);
    // Band geometry shared by this paragraph's own float (below) and the
    // `ActiveFloat` it may leave for following paragraphs.
    let own_float: Option<(f32, f32, bool)> = float_plan.as_ref().map(|(_, p)| {
        let inset = p.indent_start_delta + p.indent_end_delta;
        (inset, p.height, p.indent_start_delta > 0.0)
    });
    if let Some((idx, _)) = &float_plan
        && let Some((inset, height, shift_text)) = own_float
    {
        // The banded layout path narrows the lines beside the float and reflows
        // the rest at full width (one of the deltas is zero — left vs right).
        resolved.wrap_band = Some(crate::para::WrapBand {
            inset,
            cover_height: height,
            shift_text,
        });
        images.remove(*idx);
    }

    state.cursor_y += resolved.space_before;

    if resolved.page_break_before && state.mode.is_paginated() {
        finish_page(state);
    }

    // Cross-paragraph wrap: when this paragraph has no float of its own but an
    // earlier float still extends below the cursor, narrow it to clear the
    // remaining band (the part of the float still above the paragraph top).
    if own_float.is_none()
        && let Some(af) = &state.active_float
        && state.cursor_y < af.bottom_y - 0.5
    {
        resolved.wrap_band = Some(crate::para::WrapBand {
            inset: af.inset,
            cover_height: af.bottom_y - state.cursor_y,
            shift_text: af.shift_text,
        });
    }

    // Record comment start anchors at the paragraph's top (on the final page,
    // after any page break above) for the gutter comment panel.
    super::comments_impl::record_comment_anchors(state, &effective_para.inlines);

    let mut para_layout = layout_paragraph_spelled(
        state.resources,
        &text,
        &spans,
        &resolved,
        state.content_width,
        state.display_scale,
        state.options.preserve_for_editing,
        state.options.spell.as_ref(),
    );

    // Picture bullet (feature 5.4): place the label image in the hanging label
    // box on line 0. Injected into the paragraph's items so it translates with
    // the paragraph on placement.
    if let Some(src) = &marker.bullet_src
        && let Some(item) =
            super::flow_list_marker::picture_bullet_item(src, &resolved, &para_layout)
    {
        para_layout.items.push(item);
    }

    // ── Inline image placement (gap #9) ──────────────────────────────────────
    // TODO(inline-image-flow): no Parley inline image boxes; images are a
    // block-level prefix and existing items shift down to make room.
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
        image_items.append(&mut para_layout.items);
        para_layout.items = image_items;
    }

    // Emit the float beside the wrapped text; a float taller than its text
    // becomes an `ActiveFloat` so *following* paragraphs wrap its remainder.
    if let Some((_, placement)) = float_plan {
        para_layout.items.push(placement.item);
    }

    // The paragraph's content top in page coordinates (where the float image's
    // own top sits), captured before placement may advance/split the cursor.
    let para_top = state.cursor_y;
    let page_before = state.page_number;

    place_paragraph_layout(state, &resolved, para_layout, block_index);

    // Maintain the cross-paragraph float band.
    if state.page_number != page_before {
        // The paragraph crossed a page; wrap does not span pages.
        state.active_float = None;
    } else if let Some((inset, height, shift_text)) = own_float {
        // A float taller than its anchoring paragraph keeps wrapping below.
        let bottom_y = para_top + height;
        state.active_float =
            (bottom_y > state.cursor_y + 0.5).then_some(super::float_impl::ActiveFloat {
                bottom_y,
                inset,
                shift_text,
            });
    } else if let Some(af) = &state.active_float {
        // Inherited float: drop it once this paragraph reaches its bottom.
        if state.cursor_y >= af.bottom_y - 0.5 {
            state.active_float = None;
        }
    }

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
        if state.options.preserve_for_editing {
            // origin (dx, dy) matches the item translation below (lists indent dx).
            push_editing_para(state, block_index, Arc::new(para_layout.clone()), (dx, dy));
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
                break_column(state);
                state.cursor_y += resolved.space_before;
            }
            // Fits on current (or freshly flushed) column/page.
            let dy = state.cursor_y;
            let dx = state.current_indent;
            if state.options.preserve_for_editing {
                push_editing_para(state, block_index, Arc::new(para_layout.clone()), (0.0, dy));
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
            crate::resolve::para_map::para_keep_with_next(p, state.catalog)
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
        state
            .warnings
            .push(LayoutWarning::KeepWithNextChainTruncated {
                start_block: start,
                chain_length: natural_len,
            });
        tracing::warn!(
            start_block = start,
            "keep-with-next chain exceeds 5; truncating"
        );
    }

    // Speculatively layout all chain blocks to measure total height.
    let chain = build_chain_layouts(state, blocks, start, chain_end);
    let total_h: f32 = chain
        .iter()
        .map(|(r, l)| r.space_before + l.height + r.space_after)
        .sum();
    let chain_len = chain_end - start + 1;

    if total_h > state.page_content_height {
        return place_chain_too_tall(state, chain, start, chain_end, total_h);
    }

    let available = state.page_content_height - state.cursor_y;
    if total_h > available && state.cursor_y > 0.0 {
        break_column(state);
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
            // Convert every block type to an effective StyledParagraph so that
            // all chain members receive a proper parley_layout. Without this,
            // Heading (and other non-StyledPara) blocks in a chain end up with
            // parley_layout=None, causing cursor_rect to return None.
            let effective_para: Option<StyledParagraph> = match &blocks[idx] {
                Block::StyledPara(p) => Some(p.clone()),
                Block::Heading(lvl, attr, inlines) => {
                    Some(super::synthesize_heading_para(*lvl, attr, inlines))
                }
                Block::Para(inlines) | Block::Plain(inlines) => {
                    Some(super::synthesize_plain_para(inlines))
                }
                _ => None,
            };

            if let Some(para) = effective_para {
                let resolved = resolve_para_props(&para, state.catalog);
                let mut temp_counter = state.note_counter;
                let (text, spans, _images, _notes) = crate::resolve::flatten_paragraph_with_base(
                    &para,
                    state.catalog,
                    &mut temp_counter,
                    state.cell_char_defaults.as_ref(),
                );
                let layout = layout_paragraph_spelled(
                    state.resources,
                    &text,
                    &spans,
                    &resolved,
                    state.content_width,
                    state.display_scale,
                    state.options.preserve_for_editing,
                    state.options.spell.as_ref(),
                );
                (resolved, layout)
            } else {
                // Non-text block (HR, table, etc.): contribute zero height.
                (
                    ResolvedParaProps::default(),
                    ParagraphLayout {
                        height: 0.0,
                        width: 0.0,
                        items: vec![],
                        first_baseline: 0.0,
                        last_baseline: 0.0,
                        line_boundaries: vec![],
                        parley_layout: None,
                        orig_to_clean: vec![0],
                        clean_to_orig: vec![0],
                        indent_start: 0.0,
                        indent_hanging: 0.0,
                        drop_lines: 0,
                        drop_shift: 0.0,
                    },
                )
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

    state
        .warnings
        .push(LayoutWarning::KeepWithNextChainTooTall {
            start_block: start,
            break_at,
        });
    tracing::warn!(
        start_block = start,
        end_block = chain_end,
        "keep-with-next chain too tall for one page; breaking at block {break_at}"
    );

    if state.cursor_y > 0.0 {
        break_column(state);
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
