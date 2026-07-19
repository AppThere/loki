// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Keep-with-next chain flowing (ADR 004 §4), split out of `flow_para.rs` for
//! the 300-line ceiling: `flow_keep_with_next_chain` scans a run of
//! `keep_with_next` blocks, speculatively lays them out, and places them
//! together (or breaks a too-tall chain at the best prefix). Re-exported from
//! the parent (`para_impl`) for `flow.rs`; reaches placement via
//! `super::place_paragraph_layout` and the block synthesizers via
//! `super::super::` (the `flow` module).

use loki_doc_model::content::block::{Block, StyledParagraph};

use crate::para::{ParagraphLayout, ResolvedParaProps, layout_paragraph_spelled};
use crate::resolve::{CollectedNote, resolve_para_props};

use super::{FlowState, LayoutWarning, break_column, finish_page, place_paragraph_layout};

/// A speculatively-built chain member: its resolved props, laid-out paragraph,
/// and the footnotes/endnotes it collected (committed to `pending_footnotes`
/// only when the block is actually placed, so a re-flowed too-tall suffix does
/// not double-collect).
type ChainEntry = (ResolvedParaProps, ParagraphLayout, Vec<CollectedNote>);

/// Maximum keep-with-next chain length before truncation (ADR 004 §4).
const CHAIN_LIMIT: usize = 5;

/// Handle a `keep_with_next` chain of top-level section blocks.
///
/// Scans forward from `start`, speculatively lays out all chain blocks, then
/// decides whether to flush the current page before placing the chain.
///
/// Returns the number of section blocks consumed so the caller can skip them.
pub(crate) fn flow_keep_with_next_chain(
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
        // Only extend the chain into a block this function can actually lay out
        // and place. A non-paragraph block (table, rule, nested list) must flow
        // through the normal `flow_block` dispatch — pulling it into the chain
        // would place it as a zero-height empty paragraph and silently drop its
        // content. This is what dropped a table that immediately followed its
        // `keepNext` caption (the ubiquitous "Table N" caption pattern).
        // TODO(kwn-table): keep a caption *visually* with its table across a
        //   page break too — needs real table measurement inside the chain.
        if !is_chain_compatible(&blocks[chain_end + 1]) {
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
        .map(|(r, l, _)| r.space_before + l.height + r.space_after)
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

/// Whether a block can be laid out as a member of a keep-with-next chain.
///
/// Mirrors the conversion in [`build_chain_layouts`]: only paragraph-like blocks
/// have a Parley layout the chain can measure and place. Other blocks (tables,
/// rules, lists) flow through the normal dispatch instead, so the chain must not
/// absorb them.
fn is_chain_compatible(block: &Block) -> bool {
    matches!(
        block,
        Block::StyledPara(_) | Block::Heading(..) | Block::Para(_) | Block::Plain(_)
    )
}

/// Speculatively lay out blocks `start..=end`, returning each member's props,
/// layout, and collected notes. A running note counter is threaded across the
/// blocks so numbering is sequential; `state.note_counter` is **not** advanced
/// here (placement commits it) so this speculative pass has no side effects.
fn build_chain_layouts<'s>(
    state: &mut FlowState<'s>,
    blocks: &[Block],
    start: usize,
    end: usize,
) -> Vec<ChainEntry> {
    // Seed from the live counter but keep a local copy: the numbers baked into
    // the layouts here are re-derived at placement, which advances the real one.
    let mut counter = state.note_counter;
    let mut out = Vec::with_capacity(end - start + 1);
    for block in &blocks[start..=end] {
        // Convert every block type to an effective StyledParagraph so that
        // all chain members receive a proper parley_layout. Without this,
        // Heading (and other non-StyledPara) blocks in a chain end up with
        // parley_layout=None, causing cursor_rect to return None.
        let effective_para: Option<StyledParagraph> = match block {
            Block::StyledPara(p) => Some(p.clone()),
            Block::Heading(lvl, attr, inlines) => {
                Some(super::super::synthesize_heading_para(*lvl, attr, inlines))
            }
            Block::Para(inlines) | Block::Plain(inlines) => {
                Some(super::super::synthesize_plain_para(inlines))
            }
            _ => None,
        };

        if let Some(para) = effective_para {
            let resolved = resolve_para_props(&para, state.catalog);
            let (text, spans, images, notes) = crate::resolve::flatten_paragraph_with_base(
                &para,
                state.catalog,
                &mut counter,
                state.cell_char_defaults.as_ref(),
            );
            let mut layout = layout_paragraph_spelled(
                state.resources,
                &text,
                &spans,
                &resolved,
                state.content_width,
                state.display_scale,
                state.options.preserve_for_editing,
                state.options.spell.as_ref(),
            );
            // Block-stack any inline images (a captioned figure with
            // `keepNext` on its image paragraph would otherwise vanish —
            // the chain path formerly discarded the collected images).
            let overlay = super::stack_block_images(&mut layout, &images, state.content_width);
            super::apply_overlay_images(&mut layout, overlay);
            out.push((resolved, layout, notes));
        } else {
            // Non-text block (HR, table, etc.): contribute zero height.
            out.push((
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
                Vec::new(),
            ));
        }
    }
    out
}

/// Commit a placed chain block's notes: tag them with their owning block and
/// per-block order, hand them to `pending_footnotes`, and advance the real note
/// counter (mirrors `flow_paragraph`, so a `keepNext` caption's footnote is
/// rendered rather than dropped).
fn collect_chain_notes(state: &mut FlowState, mut notes: Vec<CollectedNote>, block_index: usize) {
    if notes.is_empty() {
        return;
    }
    for (i, note) in notes.iter_mut().enumerate() {
        note.owner_block_index = block_index;
        note.note_in_block = i;
    }
    state.note_counter += notes.len() as u32;
    state.pending_footnotes.extend(notes);
}

/// Place chain blocks in order, adding `space_before` to `cursor_y` before each.
fn place_chain_blocks(state: &mut FlowState, chain: Vec<ChainEntry>, start: usize) {
    for (i, (resolved, layout, notes)) in chain.into_iter().enumerate() {
        state.cursor_y += resolved.space_before;
        if resolved.page_break_before && state.mode.is_paginated() {
            finish_page(state);
        }
        collect_chain_notes(state, notes, start + i);
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
    chain: Vec<ChainEntry>,
    start: usize,
    chain_end: usize,
    _total_h: f32,
) -> usize {
    // Find largest prefix whose total height fits on one fresh page.
    let mut prefix_h = 0.0f32;
    let mut last_fits = start;
    for (i, (resolved, layout, _)) in chain.iter().enumerate() {
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
    for (i, (resolved, layout, notes)) in chain.into_iter().enumerate() {
        if start + i > last_fits {
            // Un-placed suffix falls back to the caller's main loop, which
            // re-flows it (re-collecting its notes) — so drop these here.
            break;
        }
        state.cursor_y += resolved.space_before;
        if resolved.page_break_before && state.mode.is_paginated() {
            finish_page(state);
        }
        collect_chain_notes(state, notes, start + i);
        place_paragraph_layout(state, &resolved, layout, start + i);
    }
    consumed
}
