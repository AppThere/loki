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
    BreakCause, FlowState, columns_impl, float_impl, flow_block, flow_keep_with_next_chain,
    para_between,
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
        pending_endnotes: Vec::new(),
        footnote_reserved: 0.0,
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
        last_space_after: 0.0,
        suppress_space_before: false,
    }
}

impl FlowState<'_> {
    /// Lowest `y` body content may reach before breaking — `page_content_height`
    /// minus this page's [`footnote_reserved`](FlowState::footnote_reserved),
    /// floored at `cursor_y` (so an over-full page never reports negative room).
    /// Used only by the "space remaining on this page" break checks.
    pub(super) fn content_bottom(&self) -> f32 {
        (self.page_content_height - self.footnote_reserved).max(self.cursor_y)
    }

    /// Opens a block by advancing the cursor for its `space_before`,
    /// **collapsed** against the `space_after` the previous block already
    /// contributed: the gap between two blocks is `max(after, before)`, not
    /// their sum.
    ///
    /// # Why max rather than sum
    ///
    /// This is Word's behaviour, and it was measured rather than assumed:
    /// two paragraphs with `after = 12pt` on the first, exported to PDF by
    /// Word 16.0 and rasterised at 144 dpi, leave the same 42 px gap whether
    /// the second declares `before = 0` or `before = 6pt`. Summing instead
    /// leaked the smaller of the two at *every* boundary, and because the
    /// error accumulates down the page it eventually overflows one — Loki
    /// needed 15 pages for a 14-page Word document, after which every page
    /// compared against unrelated content.
    ///
    /// Applying only the excess (`before - already`) is what makes this
    /// `max()`: the previous block's `space_after` has already moved the
    /// cursor, so adding the difference tops it up to the larger of the two
    /// and adds nothing when it is already the larger.
    ///
    /// At the top of a page or column reached by a [`BreakCause::Flow`] break,
    /// `space_before` is **suppressed entirely** rather than collapsed — the
    /// page margin already provides the separation the spacing exists to
    /// create. A [`BreakCause::Forced`] break keeps the ordinary collapse; see
    /// [`BreakCause`] for the measurements that separate the two.
    pub(super) fn advance_space_before(&mut self, before: f32) {
        if std::mem::take(&mut self.suppress_space_before) {
            self.last_space_after = 0.0;
            return;
        }
        self.cursor_y += (before - self.last_space_after).max(0.0);
        self.last_space_after = 0.0;
    }

    /// Closes a block by advancing the cursor for its `space_after` and
    /// remembering it, so the next block's `space_before` collapses against it.
    pub(super) fn apply_space_after(&mut self, after: f32) {
        self.cursor_y += after;
        self.last_space_after = after;
    }

    /// Settles paragraph-spacing collapse across a page or column boundary,
    /// according to [why the boundary happened](BreakCause).
    ///
    /// A [`Flow`](BreakCause::Flow) break forgets the pending `space_after` and
    /// suppresses the next block's `space_before`: there is no longer a
    /// preceding block on this page to collapse against, and Word drops the
    /// spacing outright.
    ///
    /// A [`Forced`](BreakCause::Forced) break leaves **both** alone, which is
    /// what makes the next block's `space_before` collapse against the
    /// preceding block's `space_after` exactly as it would mid-page. Clearing
    /// either one here would silently turn Word's `max(0, before - after)` into
    /// `before` (clearing `last_space_after`) or into `0` (suppressing).
    pub(super) fn end_page_at(&mut self, cause: BreakCause) {
        if cause == BreakCause::Flow {
            self.last_space_after = 0.0;
            self.suppress_space_before = true;
        }
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
