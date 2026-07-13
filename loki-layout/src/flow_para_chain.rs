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
use crate::resolve::resolve_para_props;

use super::{FlowState, LayoutWarning, break_column, finish_page, place_paragraph_layout};

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
                    Some(super::super::synthesize_heading_para(*lvl, attr, inlines))
                }
                Block::Para(inlines) | Block::Plain(inlines) => {
                    Some(super::super::synthesize_plain_para(inlines))
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
