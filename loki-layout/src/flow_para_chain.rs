// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Keep-with-next chain flowing (ADR 004 §4), split out of `flow_para.rs` for
//! the 300-line ceiling: `flow_keep_with_next_chain` scans a run of
//! `keep_with_next` blocks, speculatively lays them out, and places them
//! together (or breaks a too-tall chain at the best prefix). Re-exported from
//! the parent (`para_impl`) for `flow.rs`; reaches placement via
//! `super::place_paragraph_layout` and the block synthesizers via
//! `super::super::` (the `flow` module).

use std::sync::Arc;

use loki_doc_model::content::block::{Block, StyledParagraph};

use crate::para::{ByteIndexMap, ParagraphLayout, ResolvedParaProps, layout_paragraph_spelled};
use crate::resolve::{CollectedNote, resolve_para_props};

use super::{FlowState, LayoutWarning, break_column};

#[path = "flow_para_chain_place.rs"]
mod place;
use place::{place_chain_blocks, place_chain_too_tall};

/// A speculatively-built chain member: its resolved props, laid-out paragraph,
/// and the footnotes/endnotes it collected (committed to `pending_footnotes`
/// only when the block is actually placed, so a re-flowed too-tall suffix does
/// not double-collect).
///
/// The layout is the shaping cache's `Arc` (S9-1); a chain member with images
/// takes a private copy via `Arc::make_mut`, the rest share the entry.
pub(super) type ChainEntry = (ResolvedParaProps, Arc<ParagraphLayout>, Vec<CollectedNote>);

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

    let available = state.content_bottom() - state.cursor_y;
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
            let mut resolved = resolve_para_props(&para, state.catalog);
            let (text, spans, images, notes, para_mark_size) =
                crate::resolve::flatten_paragraph_with_base(
                    &para,
                    state.catalog,
                    &mut counter,
                    state.cell_char_defaults.as_ref(),
                    state.options.revision_display,
                );
            // Sizes an empty paragraph's line (`default_font_size`).
            resolved.default_font_size = para_mark_size;
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
            // Block-stack inline images (a captioned figure with `keepNext` on
            // its image paragraph would otherwise vanish — this path formerly
            // discarded them). Copy-on-write only when there are any (S9-1).
            if !images.is_empty() {
                let l = Arc::make_mut(&mut layout);
                let fit = state.mode.fits_oversized_to_column();
                let w = state.content_width;
                let overlay = super::stack_block_images(l, &images, w, fit, resolved.alignment);
                super::apply_overlay_images(l, overlay);
            }
            out.push((resolved, layout, notes));
        } else {
            // Non-text block (HR, table, etc.): contribute zero height.
            out.push((
                ResolvedParaProps::default(),
                Arc::new(ParagraphLayout {
                    height: 0.0,
                    width: 0.0,
                    items: vec![],
                    first_baseline: 0.0,
                    last_baseline: 0.0,
                    line_boundaries: vec![],
                    parley_layout: None,
                    orig_to_clean: ByteIndexMap::Identity { len: 1 },
                    clean_to_orig: ByteIndexMap::Identity { len: 1 },
                    indent_start: 0.0,
                    indent_hanging: 0.0,
                    drop_lines: 0,
                    drop_shift: 0.0,
                }),
                Vec::new(),
            ));
        }
    }
    out
}
