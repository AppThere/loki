// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Flow-state construction and the top-level paginated block loop, split out of
//! `flow.rs` for the 300-line ceiling. Both are re-exported from `flow.rs` and
//! used by the section entry points plus `flow_balance` / `flow_group`.

use std::collections::HashMap;

use loki_doc_model::content::block::Block;
use loki_doc_model::{Section, StyleCatalog};

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutSize};
use crate::incremental::{FlowCheckpoint, PageStart};
use crate::mode::LayoutMode;
use crate::resolve::para_map::para_keep_with_next;
use crate::resolve::pts_to_f32;

use super::{
    FlowState, columns_impl, float_impl, flow_block, flow_keep_with_next_chain, para_between,
};

/// Builds a fresh [`FlowState`] for `section` in `mode`.
///
/// Multi-column layout is a paginated-print feature: the content area is divided
/// into `count` equal columns separated by `gap`, and the flow fills each column
/// top-to-bottom before advancing to the next (then the page). Single-column and
/// non-paginated (reflow/pageless) flows use the full width.
pub(super) fn new_flow_state<'a>(
    resources: &'a mut FontResources,
    section: &'a Section,
    catalog: &'a StyleCatalog,
    mode: &'a LayoutMode,
    display_scale: f32,
    options: &'a LayoutOptions,
    comments: &'a [loki_doc_model::content::annotation::Comment],
) -> FlowState<'a> {
    let pl = &section.layout;
    let page_w = pts_to_f32(pl.page_size.width);
    let page_h = pts_to_f32(pl.page_size.height);
    let margins = LayoutInsets {
        top: pts_to_f32(pl.margins.top),
        right: pts_to_f32(pl.margins.right),
        bottom: pts_to_f32(pl.margins.bottom),
        left: pts_to_f32(pl.margins.left),
    };
    let full_content_width = match mode {
        LayoutMode::Reflow { available_width } => *available_width,
        _ => (page_w - margins.horizontal()).max(0.0),
    };
    let (columns, column_gap, column_separator, column_widths) = columns_impl::column_layout_for(
        pl.columns.as_ref(),
        full_content_width,
        mode.is_paginated(),
    );
    let content_width = column_widths[0];
    FlowState {
        resources,
        catalog,
        mode,
        display_scale,
        options,
        cursor_y: 0.0,
        content_width,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::new(page_w, page_h),
        margins,
        page_content_height: (page_h - margins.vertical()).max(0.0),
        rendering_footnotes: false,
        page_number: 1,
        warnings: Vec::new(),
        current_indent: 0.0,
        list_counters: HashMap::new(),
        prev_list_id: None,
        note_counter: 0,
        pending_footnotes: Vec::new(),
        current_paragraphs: Vec::new(),
        checkpoints: Vec::new(),
        columns,
        column_widths,
        column_gap,
        column_separator,
        col_index: 0,
        column_top_y: 0.0,
        column_item_start: 0,
        column_para_start: 0,
        comments,
        pending_comment_anchors: Vec::new(),
        break_long_words: false,
        active_float: None,
        nested_editing: None,
        staged_between: None,
        tail_candidate: None,
        cell_char_defaults: None,
        line_num: pl
            .line_numbering
            .as_ref()
            .map(super::line_numbers::LineNumberState::new),
    }
}

/// Runs the top-level paginated block loop over `blocks[start..]`.
///
/// At every *clean page top* (cursor at 0, nothing placed — i.e. between
/// top-level blocks) the position is offered to `resync`: `true` stops the
/// loop and returns `Some(block_index)` (the caller splices a reused page
/// suffix); otherwise it is recorded as a [`PageStart`] checkpoint and the
/// flow continues. Returns `None` at the end of `blocks`.
pub(super) fn run_paginated_loop(
    state: &mut FlowState,
    blocks: &[Block],
    start: usize,
    block_index_base: usize,
    mut resync: impl FnMut(usize, &FlowCheckpoint) -> bool,
) -> Option<usize> {
    let mut i = start;
    while i < blocks.len() {
        // Balancing probe (`flow_balance`, multi-column only): snapshot before
        // the block; if exactly one page flushes while it flows, this block is
        // the candidate start of the newest page (verified before use).
        let probe = (state.columns > 1).then(|| (state.snapshot_checkpoint(), state.pages.len()));
        if state.cursor_y == 0.0 && state.current_items.is_empty() {
            let cp = state.snapshot_checkpoint();
            if resync(i, &cp) {
                return Some(i);
            }
            let ps = PageStart {
                page_index: state.pages.len(),
                // Filled in by `layout_paginated_full` (flow is section-local).
                section_index: 0,
                block_index: block_index_base + i,
                checkpoint: cp,
            };
            // A clean page top is a proven candidate — it supersedes the
            // previous block's flush-derived guess.
            state.tail_candidate = Some(ps.clone());
            state.checkpoints.push(ps);
        }
        let block = &blocks[i];
        let block_i = i;
        if let Block::StyledPara(para) = block
            && para_keep_with_next(para, state.catalog)
        {
            // NOTE: `i` is the slice index (chain scanning indexes `blocks`);
            // editing indices in a kwn chain are not offset by `block_index_base`.
            let consumed = flow_keep_with_next_chain(state, blocks, i);
            i += consumed;
        } else {
            state.staged_between = para_between::stage(blocks, i, state.catalog);
            flow_block(state, block, block_index_base + i);
            i += 1;
        }
        if let Some((mut cp, pages_before)) = probe {
            if state.pages.len() == pages_before + 1 {
                // The flush bumped the page number after the snapshot; the
                // page this block starts carries the next number.
                cp.page_number += 1;
                state.tail_candidate = Some(PageStart {
                    page_index: state.pages.len(),
                    section_index: 0,
                    block_index: block_index_base + block_i,
                    checkpoint: cp,
                });
            } else if state.pages.len() > pages_before {
                state.tail_candidate = None; // block spans pages — no clean seed
            }
        }
    }
    // Reserve any float left active by the final block so the section's height
    // accounts for it.
    float_impl::reserve_active_float(state);
    None
}
