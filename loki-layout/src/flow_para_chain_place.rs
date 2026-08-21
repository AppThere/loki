// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Placement half of the keep-with-next chain, split out of
//! `flow_para_chain.rs` for the 300-line ceiling: once the parent has scanned a
//! chain and speculatively laid it out, these place it — notes committed, each
//! block opened through [`open_chain_block`], and a too-tall chain broken at its
//! best-fitting prefix. The parent keeps the scan/measure half.

use super::super::{BreakCause, break_column, finish_page, place_paragraph_layout};
use super::{ChainEntry, FlowState, LayoutWarning};
use crate::para::ResolvedParaProps;
use crate::resolve::CollectedNote;

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
    // The chain is placed as a single-page unit (it was measured to fit), so its
    // notes land on the current page — reserve their band immediately so
    // post-chain content stops above it.
    // One `super` deeper than the parent module's own reach into `flow`.
    state.footnote_reserved += super::super::super::tail::footnote_reservation(state, &notes);
    state.pending_footnotes.extend(notes);
}

/// Opens a chain block: its own page break first, then its `space_before`.
///
/// That order is `flow_para`'s, and for the same reason — spacing added to a
/// page about to be closed is spacing nobody sees, and it is the paragraph
/// *starting* the new page whose `space_before` the break decides. Both chain
/// placement loops go through here so they cannot disagree about it.
fn open_chain_block(state: &mut FlowState, resolved: &ResolvedParaProps) {
    if resolved.page_break_before && state.mode.is_paginated() {
        finish_page(state, BreakCause::Forced);
    }
    state.advance_space_before(resolved.space_before);
}

/// Place chain blocks in order, adding `space_before` to `cursor_y` before each.
pub(super) fn place_chain_blocks(state: &mut FlowState, chain: Vec<ChainEntry>, start: usize) {
    for (i, (resolved, layout, notes)) in chain.into_iter().enumerate() {
        open_chain_block(state, &resolved);
        collect_chain_notes(state, notes, start + i);
        place_paragraph_layout(state, &resolved, layout, start + i);
    }
}

/// Handle a chain that is taller than one page: find the prefix that fits,
/// emit `KeepWithNextChainTooTall`, flush if needed, place the prefix.
///
/// Returns the number of blocks consumed (the fitting prefix only; remaining
/// blocks fall back to the caller's main loop).
pub(super) fn place_chain_too_tall(
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
        open_chain_block(state, &resolved);
        collect_chain_notes(state, notes, start + i);
        place_paragraph_layout(state, &resolved, layout, start + i);
    }
    consumed
}
