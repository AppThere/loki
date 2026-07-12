// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Flow engine — places blocks sequentially and handles page breaking.
//!
//! [`flow_section`] converts a stream of [`Block`]s into positioned items.
//! In paginated mode the engine splits paragraphs at Parley line boundaries
//! and uses [`PositionedItem::ClippedGroup`] to render each page fragment
//! correctly. Page objects are built directly (no re-binning pass).
//!
//! Paragraph placement/splitting logic lives in `para_impl` (`flow_para.rs`).

#[path = "flow_balance.rs"]
mod balance;
#[path = "flow_columns.rs"]
mod columns_impl;
#[path = "flow_comments.rs"]
mod comments_impl;
#[path = "flow_editing.rs"]
mod editing;
#[path = "flow_float.rs"]
mod float_impl;
#[path = "flow_list_marker.rs"]
mod flow_list_marker;
#[path = "flow_group.rs"]
mod group;
#[path = "flow_headers.rs"]
mod headers;
#[path = "flow_page_fields.rs"]
mod page_fields;
#[path = "flow_para_between.rs"]
mod para_between;
#[path = "flow_para.rs"]
mod para_impl;
#[path = "flow_table_cells.rs"]
mod table_cells;
#[path = "flow_table_chars.rs"]
mod table_chars;
#[path = "flow_table_geom.rs"]
mod table_geom;
#[path = "flow_table_main.rs"]
mod table_main;
#[path = "flow_table_paint.rs"]
mod table_paint;
#[path = "flow_tail.rs"]
mod tail;

pub use group::flow_section_group;
pub(crate) use headers::assign_headers_footers;
pub(crate) use headers::layout_blocks_reflow;
pub(crate) use page_fields::page_layout_has_page_fields;
use tail::{
    flow_footnotes, flow_hrule, get_items_max_x, synthesize_heading_para, synthesize_plain_para,
};

use std::collections::HashMap;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::list_style::ListId;
use loki_doc_model::{Section, StyleCatalog};
use loki_primitives::units::Points;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutSize};
use crate::incremental::{FlowCheckpoint, PageStart};
use crate::items::PositionedItem;
use crate::mode::LayoutMode;
use crate::resolve::{CollectedNote, para_map::para_keep_with_next, pts_to_f32};
use crate::result::{LayoutPage, PageEditingData, PageParagraphData};

use para_impl::{flow_keep_with_next_chain, flow_paragraph};

// ── Public types ──────────────────────────────────────────────────────────────

/// Output of [`flow_section`], discriminated by layout mode.
pub enum FlowOutput {
    /// Returned when `mode.is_paginated()`. Item origins are relative to the
    /// page content-area top-left `(0, 0)` — no further translation needed.
    Pages {
        /// Completed pages with content items in page-local coordinates.
        pages: Vec<LayoutPage>,
        /// Clean-page-top checkpoints for incremental relayout (empty for
        /// nested/non-top-level flows).
        checkpoints: Vec<PageStart>,
        /// Non-fatal warnings collected during layout.
        warnings: Vec<LayoutWarning>,
    },
    /// Returned for `Pageless` and `Reflow` modes.
    Canvas {
        /// All positioned items on the single canvas.
        items: Vec<PositionedItem>,
        /// Total canvas height in points.
        height: f32,
        /// Per-paragraph editing data (canvas-local; reflow hit-testing).
        /// Empty unless `preserve_for_editing` is set.
        paragraphs: Vec<crate::result::PageParagraphData>,
        /// Non-fatal warnings collected during layout.
        warnings: Vec<LayoutWarning>,
    },
}

/// Non-fatal layout issues collected during [`flow_section`].
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum LayoutWarning {
    /// A block was too tall to fit on any page in paginated mode.
    BlockExceedsPageHeight {
        /// 0-indexed position of the block in the section.
        block_index: usize,
        /// Measured block height in points.
        block_height: f32,
    },
    /// An image src could not be resolved by the renderer.
    UnresolvedImage {
        /// Source URL or data URI that could not be resolved.
        src: String,
    },
    /// A `keep_together` paragraph was split because it exceeds full page
    /// height. The block could not be kept together on any single page.
    KeepTogetherOverride {
        /// 0-indexed position of the block in the section.
        block_index: usize,
        /// Measured block height in points.
        block_height: f32,
    },
    /// A `keep_with_next` chain was truncated at the chain limit of 5 blocks.
    KeepWithNextChainTruncated {
        /// 0-indexed position of the first block in the chain.
        start_block: usize,
        /// Number of blocks in the chain before truncation.
        chain_length: usize,
    },
    /// A `keep_with_next` chain was too tall to fit on one page; the chain
    /// was broken at the last block that fits.
    KeepWithNextChainTooTall {
        /// 0-indexed position of the first block in the chain.
        start_block: usize,
        /// Index of the block where the chain was broken.
        break_at: usize,
    },
}

// ── Private flow state ────────────────────────────────────────────────────────

pub(super) struct FlowState<'a> {
    pub(super) resources: &'a mut FontResources,
    pub(super) catalog: &'a StyleCatalog,
    pub(super) mode: &'a LayoutMode,
    pub(super) display_scale: f32,
    /// Layout options forwarded from the [`layout_document`] caller.
    pub(super) options: &'a LayoutOptions,
    /// Current y within the current page content area (or canvas).
    pub(super) cursor_y: f32,
    pub(super) content_width: f32,
    /// Items accumulating in the current page (or entire canvas for continuous).
    pub(super) current_items: Vec<PositionedItem>,
    /// Completed pages (paginated mode only).
    pub(super) pages: Vec<LayoutPage>,
    pub(super) page_size: LayoutSize,
    pub(super) margins: LayoutInsets,
    /// Height of the content area within a page (page_height − v_margins).
    pub(super) page_content_height: f32,
    /// 1-indexed current page number.
    pub(super) page_number: usize,
    /// Accumulated warnings.
    pub(super) warnings: Vec<LayoutWarning>,
    /// Accumulated horizontal indentation in points.
    pub(super) current_indent: f32,
    /// Per-list counters: `ListId` → per-level counters (`0` = uninitialised).
    pub(super) list_counters: HashMap<ListId, [u32; 9]>,
    /// `ListId` of the most recently placed list item (detects list changes).
    pub(super) prev_list_id: Option<ListId>,
    /// Footnote/endnote counter for the section (bumped by `walk_inlines`);
    /// collected notes render via `flow_footnotes`.
    pub(super) note_counter: u32,
    pub(super) pending_footnotes: Vec<CollectedNote>,
    /// Paragraph metadata for the current page (block index, layout, origin).
    pub(super) current_paragraphs: Vec<PageParagraphData>,
    /// Clean-page-top checkpoints for incremental relayout (top-level only).
    pub(super) checkpoints: Vec<PageStart>,
    /// Number of text columns (`1` = single); when `> 1`,
    /// [`content_width`](Self::content_width) is the *current* column's width.
    pub(super) columns: u8,
    /// Per-column widths in points (length `columns`; may be unequal). Column
    /// x-offsets are the running sum of preceding widths plus `column_gap` per
    /// gap. `content_width` tracks the current column's entry.
    pub(super) column_widths: Vec<f32>,
    /// Gap between adjacent columns in points (only when `columns > 1`).
    pub(super) column_gap: f32,
    /// Whether to draw a separator line between columns.
    pub(super) column_separator: bool,
    /// 0-based index of the column currently being filled.
    pub(super) col_index: u8,
    /// Content-area y where the current column band begins (`0` normally; mid-page
    /// for a `continuous` section break that starts a band below the previous one).
    pub(super) column_top_y: f32,
    /// First `current_items` index of the current column (shifted at finish).
    pub(super) column_item_start: usize,
    /// First `current_paragraphs` index of the column (parallel to above).
    pub(super) column_para_start: usize,
    /// Document comments, looked up by id for the gutter panel; empty in nested flows.
    pub(super) comments: &'a [loki_doc_model::content::annotation::Comment],
    /// Comment anchors (`id`, content-local `y`) on the current page, consumed by
    /// [`finish_page`] for the gutter comment panel.
    pub(super) pending_comment_anchors: Vec<(String, f32)>,
    /// Break over-long words to the width (`overflow-wrap: anywhere`); set
    /// while flowing table-cell content so words wrap to the column width.
    pub(super) break_long_words: bool,
    /// A float taller than its anchoring paragraph whose remaining extent the
    /// following paragraphs keep wrapping beside; cleared on page boundaries.
    pub(super) active_float: Option<float_impl::ActiveFloat>,
    /// Editing-path context for nested content (see [`editing::NestedEditing`]).
    pub(super) nested_editing: Option<editing::NestedEditing>,
    /// Between-border override for the paragraph about to flow (gap #26).
    pub(super) staged_between: Option<para_between::BetweenOverride>,
    /// Newest block observed to start a fresh page, with its pre-block resume
    /// snapshot — the last-page balancing seed (`flow_balance`; multi-column
    /// flows only). `None`d when a block spans several page advances.
    pub(super) tail_candidate: Option<PageStart>,
    /// Table-region character defaults for the cell currently flowing (4a.3);
    /// merged under the paragraph chain by `flatten_paragraph_with_base`.
    pub(super) cell_char_defaults: Option<loki_doc_model::style::props::char_props::CharProps>,
}

impl FlowState<'_> {
    /// Snapshots the resumable flow state at a clean page top.
    pub(super) fn snapshot_checkpoint(&self) -> FlowCheckpoint {
        FlowCheckpoint {
            page_number: self.page_number,
            list_counters: self.list_counters.clone(),
            prev_list_id: self.prev_list_id.clone(),
            note_counter: self.note_counter,
            current_indent: self.current_indent,
        }
    }
}

impl<'a> FlowState<'a> {
    /// Advance the counter for `list_id` at `level` and return the new value.
    ///
    /// Initialises from `start_value` on first use; resets all deeper-level
    /// counters to 0 so they re-initialise from their own `start_value` next.
    pub(super) fn advance_counter(&mut self, list_id: &ListId, level: u8, start_value: u32) -> u32 {
        let counters = self
            .list_counters
            .entry(list_id.clone())
            .or_insert([0u32; 9]);
        let lvl = level as usize;
        if counters[lvl] == 0 {
            counters[lvl] = start_value;
        } else {
            counters[lvl] += 1;
        }
        for counter in counters.iter_mut().take(9).skip(lvl + 1) {
            *counter = 0;
        }
        counters[lvl]
    }
}

// ── Flow construction & paginated loop (shared with incremental relayout) ──────

/// Resolves a section's [`SectionColumns`] into `(count, gap, separator,
/// per-column width)` for a content area `full_content_width` wide.
///
/// Multi-column layout is a paginated-print feature: the content area is divided
/// into `count` equal columns separated by `gap`, and the flow fills each column
/// top-to-bottom before advancing to the next (then the page). Single-column and
/// non-paginated (reflow/pageless) flows return the full width.
/// Builds a fresh [`FlowState`] for `section` in `mode`.
fn new_flow_state<'a>(
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
    }
}

/// Runs the top-level paginated block loop over `blocks[start..]`.
///
/// At every *clean page top* (cursor at 0, nothing placed — i.e. between
/// top-level blocks) the position is offered to `resync`: `true` stops the
/// loop and returns `Some(block_index)` (the caller splices a reused page
/// suffix); otherwise it is recorded as a [`PageStart`] checkpoint and the
/// flow continues. Returns `None` at the end of `blocks`.
fn run_paginated_loop(
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

/// Resumes a paginated body flow at `start_block` from a [`FlowCheckpoint`],
/// for incremental relayout. See [`crate::incremental`].
///
/// Returns the pages produced from `start_block` up to the end of the section or
/// the first clean page top where `resync` fires. Running to the end flushes the
/// trailing footnotes and final partial page; stopping at a resync leaves the
/// empty current page unflushed, so the caller splices the reused suffix cleanly.
#[allow(clippy::too_many_arguments)]
pub(crate) fn flow_section_resume(
    resources: &mut FontResources,
    section: &Section,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    start_block: usize,
    seed: &FlowCheckpoint,
    resync: impl FnMut(usize, &FlowCheckpoint) -> bool,
) -> crate::incremental::ResumedFlow {
    let mode = LayoutMode::Paginated;
    // Incremental resume does not render comment panels on reused pages; the
    // full relayout path does. Pass an empty comment set here.
    let mut state = new_flow_state(
        resources,
        section,
        catalog,
        &mode,
        display_scale,
        options,
        &[],
    );
    state.page_number = seed.page_number;
    state.list_counters = seed.list_counters.clone();
    state.prev_list_id = seed.prev_list_id.clone();
    state.note_counter = seed.note_counter;
    state.current_indent = seed.current_indent;

    // Incremental relayout is single-section, so block indices are section-local
    // (base 0).
    let resynced_at = run_paginated_loop(&mut state, &section.blocks, start_block, 0, resync);
    if resynced_at.is_none() {
        // Reached the end: flush trailing footnotes and the final partial page.
        // On a resync stop the current page is an empty clean page top, so it is
        // intentionally left unflushed for the caller to splice the reused suffix.
        flow_footnotes(&mut state);
        finish_page(&mut state);
    }
    crate::incremental::ResumedFlow {
        pages: state.pages,
        checkpoints: state.checkpoints,
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Flow all blocks from a section into positioned items.
///
/// Returns a [`FlowOutput`] discriminated by layout mode:
///
/// - [`FlowOutput::Pages`]: each page's items are in page-content-area-local
///   coordinates (origin `(0, 0)` at the content-area top-left); the `margins`
///   on each [`LayoutPage`] carry the insets. No caller translation needed.
/// - [`FlowOutput::Canvas`]: all items on a single canvas. In `Pageless` mode
///   items are offset by `margins.left`; in `Reflow` mode there is no offset.
pub fn flow_section(
    resources: &mut FontResources,
    section: &Section,
    catalog: &StyleCatalog,
    mode: &LayoutMode,
    display_scale: f32,
    options: &LayoutOptions,
    comments: &[loki_doc_model::content::annotation::Comment],
) -> FlowOutput {
    if mode.is_paginated() {
        // Top-level paginated flow: balances multi-column single-page sections.
        return balance::flow_paginated_balanced(
            resources,
            section,
            catalog,
            mode,
            display_scale,
            options,
            comments,
        );
    }

    let mut state = new_flow_state(
        resources,
        section,
        catalog,
        mode,
        display_scale,
        options,
        comments,
    );
    for (idx, block) in section.blocks.iter().enumerate() {
        state.staged_between = para_between::stage(&section.blocks, idx, state.catalog);
        flow_block(&mut state, block, idx);
    }
    // Reserve any float left active by the final block (continuous mode).
    float_impl::reserve_active_float(&mut state);
    flow_footnotes(&mut state);

    if matches!(mode, LayoutMode::Pageless) {
        let dx = state.margins.left;
        for item in &mut state.current_items {
            item.translate(dx, 0.0);
        }
        // Keep paragraph editing origins consistent with the shifted items.
        for para in &mut state.current_paragraphs {
            para.origin.0 += dx;
        }
    }
    FlowOutput::Canvas {
        items: state.current_items,
        height: state.cursor_y,
        paragraphs: state.current_paragraphs,
        warnings: state.warnings,
    }
}

/// Transitions an in-progress paginated [`FlowState`] into a `continuous`
/// section *on the same page*: it closes the current column band (if any) and
/// opens a fresh column band — using `section`'s column layout but the **group's**
/// page geometry (a continuous break cannot change the page size) — starting at
/// the current `cursor_y` rather than the page top.
fn begin_continuous_section(state: &mut FlowState, section: &Section) {
    // Close the previous section's column band: position its final column and
    // draw its separators. If it used more than one column the band filled the
    // page height, so drop to the page bottom (the next band starts on a new
    // page via the normal column/page break); otherwise continue just below it.
    if state.columns > 1 {
        columns_impl::position_current_column(state);
        columns_impl::emit_column_separators(state);
        if state.col_index > 0 {
            state.cursor_y = state.page_content_height;
        }
    }

    // Resolve the new section's columns against the group's (unchanged) content
    // area width.
    let full_content_width = (state.page_size.width - state.margins.horizontal()).max(0.0);
    let (columns, column_gap, column_separator, column_widths) =
        columns_impl::column_layout_for(section.layout.columns.as_ref(), full_content_width, true);
    state.columns = columns;
    state.column_gap = column_gap;
    state.column_separator = column_separator;
    state.content_width = column_widths[0];
    state.column_widths = column_widths;

    // Open the new column band at the current y (mid-page).
    state.col_index = 0;
    state.column_top_y = state.cursor_y;
    state.column_item_start = state.current_items.len();
    state.column_para_start = state.current_paragraphs.len();
}

// ── Block dispatch ────────────────────────────────────────────────────────────

pub(super) fn flow_block(state: &mut FlowState, block: &Block, idx: usize) {
    // Only consecutive plain paragraphs continue a cross-paragraph float wrap;
    // any other block clears the float (reserving its remaining height) so it
    // does not overlap the image.
    if !matches!(
        block,
        Block::StyledPara(_) | Block::Para(_) | Block::Plain(_) | Block::Heading(..)
    ) {
        float_impl::reserve_active_float(state);
    }
    match block {
        Block::StyledPara(p) => flow_paragraph(state, p, idx),
        Block::Para(i) | Block::Plain(i) => {
            flow_paragraph(state, &synthesize_plain_para(i), idx);
        }
        Block::Heading(lvl, attr, i) => {
            flow_paragraph(state, &synthesize_heading_para(*lvl, attr, i), idx);
        }
        Block::BulletList(items) => {
            let old_indent = state.current_indent;
            let list_indent = old_indent + 18.0;
            for item in items {
                for (b_idx, b) in item.iter().enumerate() {
                    if b_idx == 0
                        && let Block::StyledPara(p) = b
                    {
                        let mut p = p.clone();
                        p.inlines.insert(0, Inline::Str("•\t".into()));
                        let mut direct = p.direct_para_props.take().unwrap_or_default();
                        direct.indent_hanging = Some(Points::new(18.0));
                        direct.indent_start = Some(Points::new(list_indent as f64));
                        p.direct_para_props = Some(direct);

                        let prev_indent = state.current_indent;
                        state.current_indent = 0.0;
                        flow_paragraph(state, &p, idx);
                        state.current_indent = prev_indent;
                        continue;
                    }
                    let prev_indent = state.current_indent;
                    state.current_indent = list_indent;
                    flow_block(state, b, idx);
                    state.current_indent = prev_indent;
                }
            }
            state.current_indent = old_indent;
        }
        Block::OrderedList(attrs, items) => {
            let old_indent = state.current_indent;
            let list_indent = old_indent + 18.0;
            for (i, item) in items.iter().enumerate() {
                let marker = format!("{}.\t", attrs.start_number + i as i32);
                for (b_idx, b) in item.iter().enumerate() {
                    if b_idx == 0
                        && let Block::StyledPara(p) = b
                    {
                        let mut p = p.clone();
                        p.inlines.insert(0, Inline::Str(marker.clone()));
                        let mut direct = p.direct_para_props.take().unwrap_or_default();
                        direct.indent_hanging = Some(Points::new(18.0));
                        direct.indent_start = Some(Points::new(list_indent as f64));
                        p.direct_para_props = Some(direct);

                        let prev_indent = state.current_indent;
                        state.current_indent = 0.0;
                        flow_paragraph(state, &p, idx);
                        state.current_indent = prev_indent;
                        continue;
                    }
                    let prev_indent = state.current_indent;
                    state.current_indent = list_indent;
                    flow_block(state, b, idx);
                    state.current_indent = prev_indent;
                }
            }
            state.current_indent = old_indent;
        }
        Block::BlockQuote(blocks) => {
            let old_indent = state.current_indent;
            state.current_indent += 18.0;
            for b in blocks {
                flow_block(state, b, idx);
            }
            state.current_indent = old_indent;
        }
        Block::Div(_, blocks) | Block::Figure(_, _, blocks) => flow_blocks(state, blocks, idx),
        Block::Table(tbl) => table_main::flow_table(state, tbl, idx),
        Block::HorizontalRule => flow_hrule(state),
        Block::TableOfContents(toc) => flow_blocks(state, &toc.body, idx),
        Block::Index(index) => flow_blocks(state, &index.body, idx),
        _ => {}
    }
}

/// Flows child blocks at the parent's `idx` (Div/Figure bodies, TOC/index snapshots).
fn flow_blocks(state: &mut FlowState, blocks: &[Block], idx: usize) {
    for (i, b) in blocks.iter().enumerate() {
        state.staged_between = para_between::stage(blocks, i, state.catalog);
        flow_block(state, b, idx);
    }
}

// ── Page management ───────────────────────────────────────────────────────────

pub(super) fn finish_page(state: &mut FlowState) {
    // Position + separate the used columns, then reset for the next page.
    columns_impl::position_current_column(state);
    columns_impl::emit_column_separators(state);
    state.col_index = 0;
    state.column_top_y = 0.0;
    state.column_item_start = 0;
    state.column_para_start = 0;

    // Lay out the gutter comment panel for any comments anchored on this page.
    let comment_items = comments_impl::layout_comment_panel(state);

    let page = LayoutPage {
        page_number: state.page_number,
        page_size: state.page_size,
        margins: crate::paginate_blanks::mirrored_margins(
            state.margins,
            state.page_number,
            state.options.mirror_margins,
        ),
        content_items: std::mem::take(&mut state.current_items),
        header_items: vec![],
        footer_items: vec![],
        comment_items,
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: if state.options.preserve_for_editing {
            Some(PageEditingData {
                paragraphs: state.current_paragraphs.clone(),
            })
        } else {
            None
        },
    };
    state.pages.push(page);
    state.page_number += 1;
    state.current_paragraphs.clear();
    state.cursor_y = 0.0;
    // Cross-paragraph float wrap does not continue onto the next page.
    state.active_float = None;
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
