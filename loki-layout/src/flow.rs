// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Flow engine — places blocks sequentially and handles page breaking.
//!
//! [`flow_section`] converts a stream of [`Block`]s into positioned items.
//! In paginated mode the engine splits paragraphs at Parley line boundaries
//! and uses [`PositionedItem::ClippedGroup`] to render each page fragment
//! correctly. Page objects are built directly (no re-binning pass).
//!
//! Paragraph placement, splitting, and keep-with-next chain logic live in
//! the `para_impl` submodule (`flow_para.rs`).

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
#[path = "flow_page_fields.rs"]
mod page_fields;
#[path = "flow_para.rs"]
mod para_impl;
#[path = "flow_table_cells.rs"]
mod table_cells;
#[path = "flow_table_geom.rs"]
mod table_geom;
#[path = "flow_table_paint.rs"]
mod table_paint;

pub(crate) use page_fields::page_layout_has_page_fields;

use std::collections::HashMap;

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::header_footer::HeaderFooter;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::style::list_style::ListId;
use loki_doc_model::{NodeAttr, Section, StyleCatalog};
use loki_primitives::units::Points;

use crate::LayoutOptions;
use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutRect, LayoutSize};
use crate::incremental::{FlowCheckpoint, PageStart};
use crate::items::{PositionedItem, PositionedRect};
use crate::mode::LayoutMode;
use crate::resolve::{CollectedNote, pts_to_f32, resolve_para_props};
use crate::result::{LayoutPage, PageEditingData, PageParagraphData};
use crate::table_shading::{resolve_table_style, table_look};

use para_impl::{flow_keep_with_next_chain, flow_paragraph};

// ── Public types ──────────────────────────────────────────────────────────────

/// Output of [`flow_section`], discriminated by layout mode.
pub enum FlowOutput {
    /// Returned when `mode.is_paginated()`.
    ///
    /// Item origins in each page are relative to the page content-area
    /// top-left `(0, 0)` — no further translation is needed by the caller.
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
        /// Per-paragraph editing data (canvas-local origins). Empty unless
        /// `preserve_for_editing` is set. Used for reflow hit-testing/cursor.
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
    /// Per-list counters: `ListId` → 9-element array (index = level, value =
    /// current counter; `0` = not yet initialised).
    pub(super) list_counters: HashMap<ListId, [u32; 9]>,
    /// `ListId` of the most recently placed list item (to detect list changes).
    pub(super) prev_list_id: Option<ListId>,
    /// Footnote/endnote counter for the current section (bumped by `walk_inlines`).
    pub(super) note_counter: u32,
    /// Footnotes/endnotes collected this section, rendered by `flow_footnotes`.
    pub(super) pending_footnotes: Vec<CollectedNote>,
    /// Paragraph metadata for the current page (block index, layout, origin).
    pub(super) current_paragraphs: Vec<PageParagraphData>,
    /// Clean-page-top checkpoints for incremental relayout; populated only by the
    /// top-level paginated loop (empty in nested flows).
    pub(super) checkpoints: Vec<PageStart>,
    /// Number of text columns (`1` = single); when `> 1`,
    /// [`content_width`](Self::content_width) is the *current* column's width.
    pub(super) columns: u8,
    /// Per-column widths in points (length `columns`; may be unequal). Column
    /// x-offsets are the running sum of preceding widths plus `column_gap` per
    /// gap. `content_width` tracks the current column's entry.
    pub(super) column_widths: Vec<f32>,
    /// Gap between adjacent columns in points (meaningful only when `columns > 1`).
    pub(super) column_gap: f32,
    /// Whether to draw a separator line between columns.
    pub(super) column_separator: bool,
    /// 0-based index of the column currently being filled.
    pub(super) col_index: u8,
    /// Content-area y where the current column band begins (`0` normally; mid-page
    /// for a `continuous` section break that starts a band below the previous one).
    pub(super) column_top_y: f32,
    /// First `current_items` index of the current column (shifted to its x offset
    /// when the column finishes).
    pub(super) column_item_start: usize,
    /// First `current_paragraphs` index of the current column (parallel to
    /// `column_item_start`).
    pub(super) column_para_start: usize,
    /// Document comments, looked up by id for the gutter panel; empty in nested flows.
    pub(super) comments: &'a [loki_doc_model::content::annotation::Comment],
    /// Comment anchors (`id`, content-local `y`) on the current page, consumed by
    /// [`finish_page`] for the gutter comment panel.
    pub(super) pending_comment_anchors: Vec<(String, f32)>,
    /// Break over-long words to the available width (CSS `overflow-wrap: anywhere`);
    /// set while flowing table-cell content so words wrap to the column width.
    pub(super) break_long_words: bool,
    /// A float taller than its anchoring paragraph whose remaining extent the
    /// following paragraphs keep wrapping beside. Set by `para_impl::flow_paragraph`;
    /// cleared by `float_impl::reserve_active_float` and on every page boundary.
    pub(super) active_float: Option<float_impl::ActiveFloat>,
    /// Editing-path context for nested content (see [`editing::NestedEditing`]).
    pub(super) nested_editing: Option<editing::NestedEditing>,
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
    }
}

/// Runs the top-level paginated block loop over `blocks[start..]`.
///
/// At every *clean page top* (content cursor at 0, no items placed yet, i.e.
/// between top-level blocks) it offers the position to `resync`: if `resync`
/// returns `true` the loop stops and returns `Some(block_index)` (the caller
/// splices a reused page suffix from there); otherwise the position is recorded
/// as a [`PageStart`] checkpoint and flowing continues. Returns `None` when the
/// loop reaches the end of `blocks`.
fn run_paginated_loop(
    state: &mut FlowState,
    blocks: &[Block],
    start: usize,
    block_index_base: usize,
    mut resync: impl FnMut(usize, &FlowCheckpoint) -> bool,
) -> Option<usize> {
    let mut i = start;
    while i < blocks.len() {
        if state.cursor_y == 0.0 && state.current_items.is_empty() {
            let cp = state.snapshot_checkpoint();
            if resync(i, &cp) {
                return Some(i);
            }
            state.checkpoints.push(PageStart {
                page_index: state.pages.len(),
                // section_index is filled in by `layout_paginated_full`; the
                // flow does not know its document-global section position.
                section_index: 0,
                block_index: block_index_base + i,
                checkpoint: cp,
            });
        }
        let block = &blocks[i];
        if let Block::StyledPara(para) = block
            && resolve_para_props(para, state.catalog).keep_with_next
        {
            // NOTE: `i` is the slice index (chain scanning indexes `blocks`), so
            // editing block indices inside a keep-with-next chain are not offset
            // by `block_index_base` — only matters for a kwn chain in a live-editor continuous section (rare).
            let consumed = flow_keep_with_next_chain(state, blocks, i);
            i += consumed;
            continue;
        }
        flow_block(state, block, block_index_base + i);
        i += 1;
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

/// Flows a **group** of sections that share pages: the first section starts the
/// page sequence, and every subsequent (`continuous`) member continues on the
/// same page, switching column layout mid-page via [`begin_continuous_section`].
/// Page geometry and headers/footers come from the group's first section.
///
/// Paginated mode only — the non-paginated (reflow/pageless) path flows each
/// section independently (continuous-scroll has no pages to share). Editing
/// block indices are group-local; the caller globalises them per section.
pub fn flow_section_group(
    resources: &mut FontResources,
    sections: &[&Section],
    catalog: &StyleCatalog,
    mode: &LayoutMode,
    display_scale: f32,
    options: &LayoutOptions,
    comments: &[loki_doc_model::content::annotation::Comment],
) -> FlowOutput {
    debug_assert!(mode.is_paginated(), "flow_section_group is paginated-only");
    let primary = sections[0];
    let mut state = new_flow_state(
        resources,
        primary,
        catalog,
        mode,
        display_scale,
        options,
        comments,
    );

    let mut block_base = 0usize;
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            begin_continuous_section(&mut state, section);
        }
        run_paginated_loop(&mut state, &section.blocks, 0, block_base, |_, _| false);
        block_base += section.blocks.len();
    }

    flow_footnotes(&mut state);
    finish_page(&mut state);
    FlowOutput::Pages {
        pages: state.pages,
        checkpoints: state.checkpoints,
        warnings: state.warnings,
    }
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
        Block::Table(tbl) => flow_table(state, tbl, idx),
        Block::HorizontalRule => flow_hrule(state),
        Block::TableOfContents(toc) => flow_blocks(state, &toc.body, idx),
        Block::Index(index) => flow_blocks(state, &index.body, idx),
        _ => {}
    }
}

/// Flows child blocks at the parent's `idx` (Div/Figure bodies, TOC/index snapshots).
fn flow_blocks(state: &mut FlowState, blocks: &[Block], idx: usize) {
    for b in blocks {
        flow_block(state, b, idx);
    }
}

// ── Page management ───────────────────────────────────────────────────────────

pub(super) fn finish_page(state: &mut FlowState) {
    // Position the final column's content, draw separators for the columns that
    // were used, then reset the column tracker so the next page starts at 0.
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
        margins: state.margins,
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

// ── Header / footer layout helpers ───────────────────────────────────────────

/// Lay out `blocks` in reflow mode using `available_width`.
///
/// Returns the positioned items (in `(0,0)`-origin canvas coordinates) and the
/// total canvas height. Items have no Y offset applied — the caller translates
/// them to page-local coordinates.
fn layout_blocks_reflow(
    resources: &mut FontResources,
    blocks: &[Block],
    catalog: &StyleCatalog,
    available_width: f32,
    display_scale: f32,
    field_context: Option<crate::FieldContext>,
) -> (Vec<PositionedItem>, f32) {
    use crate::LayoutOptions;
    let mut blocks = blocks.to_vec();
    // Substitute PAGE / NUMPAGES fields with their resolved values before
    // layout — the blocks are already a per-call clone, so this never
    // mutates the document.
    if let Some(ctx) = field_context {
        page_fields::substitute_page_fields(&mut blocks, &ctx);
    }
    let synthetic = Section {
        layout: PageLayout::default(),
        blocks,
        start: loki_doc_model::layout::section::SectionStart::default(),
        page_style: None,
        extensions: ExtensionBag::default(),
    };
    let mode = LayoutMode::Reflow { available_width };
    let options = LayoutOptions::default(); // headers/footers read-only here
    match flow_section(
        resources,
        &synthetic,
        catalog,
        &mode,
        display_scale,
        &options,
        &[],
    ) {
        FlowOutput::Canvas { items, height, .. } => (items, height),
        FlowOutput::Pages { .. } => unreachable!("Reflow mode always returns Canvas"),
    }
}

/// Populate header/footer items for each page in `pages`.
///
/// Variants without PAGE / NUMPAGES fields are laid out once (in reflow mode)
/// and cloned onto each page. Variants containing page fields are re-laid-out
/// per page with a [`crate::FieldContext`] carrying the real page number and
/// `total_page_count`, so "Page X of Y" chrome renders correctly.
///
/// Items are translated to page-local coords: header top `margins.header`;
/// footer top `page_height - margins.footer - footer_height`.
pub(crate) fn assign_headers_footers(
    pages: &mut [LayoutPage],
    layout: &PageLayout,
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    total_page_count: u32,
) {
    let content_width = pages
        .first()
        .map(|p| (p.page_size.width - p.margins.horizontal()).max(0.0))
        .unwrap_or(0.0);

    // Lay out a variant once when it has no page fields; `None` marks
    // variants that must be re-laid-out per page.
    let mut lay_static = |hf: &HeaderFooter| -> Option<(Vec<PositionedItem>, f32)> {
        if page_fields::blocks_contain_page_field(&hf.blocks) {
            None
        } else {
            Some(layout_blocks_reflow(
                resources,
                &hf.blocks,
                catalog,
                content_width,
                display_scale,
                None,
            ))
        }
    };

    let hdr_default = layout.header.as_ref().map(&mut lay_static);
    let hdr_first = layout.header_first.as_ref().map(&mut lay_static);
    let hdr_even = layout.header_even.as_ref().map(&mut lay_static);
    let ftr_default = layout.footer.as_ref().map(&mut lay_static);
    let ftr_first = layout.footer_first.as_ref().map(&mut lay_static);
    let ftr_even = layout.footer_even.as_ref().map(&mut lay_static);

    use crate::resolve::pts_to_f32;
    let hdr_margin_y = pts_to_f32(layout.margins.header);
    let ftr_margin = pts_to_f32(layout.margins.footer);
    let left_margin = pts_to_f32(layout.margins.left);

    // Selects the variant for page `pn`: (source blocks, pre-laid items).
    // `pre` is `None` when the variant contains page fields and must be
    // re-laid-out for each page.
    #[allow(clippy::type_complexity)] // local helper; aliasing hides intent
    fn select<'a>(
        pn: usize,
        first_src: &'a Option<HeaderFooter>,
        first_pre: &'a Option<Option<(Vec<PositionedItem>, f32)>>,
        even_src: &'a Option<HeaderFooter>,
        even_pre: &'a Option<Option<(Vec<PositionedItem>, f32)>>,
        def_src: &'a Option<HeaderFooter>,
        def_pre: &'a Option<Option<(Vec<PositionedItem>, f32)>>,
    ) -> Option<(&'a HeaderFooter, &'a Option<(Vec<PositionedItem>, f32)>)> {
        if pn == 1 && first_src.is_some() {
            first_src.as_ref().zip(first_pre.as_ref())
        } else if pn.is_multiple_of(2) && even_src.is_some() {
            even_src.as_ref().zip(even_pre.as_ref())
        } else {
            def_src.as_ref().zip(def_pre.as_ref())
        }
    }

    // First physical page of this section, used to offset the displayed number
    // when the section restarts numbering (w:pgNumType @w:start).
    let section_first_pn = pages.first().map(|p| p.page_number).unwrap_or(1);

    for page in pages.iter_mut() {
        let page_h = page.page_size.height;
        let pn = page.page_number;
        // Apply the section restart: the section's first page shows `start`, and
        // following pages increment from there. Absent a restart, use the
        // document-global physical page number.
        let display_pn = match layout.page_number_start {
            Some(start) => start as usize + pn.saturating_sub(section_first_pn),
            None => pn,
        };
        let ctx = crate::FieldContext {
            page_number: display_pn as u32,
            page_count: total_page_count,
            number_format: layout.page_number_format,
        };

        let hdr = select(
            pn,
            &layout.header_first,
            &hdr_first,
            &layout.header_even,
            &hdr_even,
            &layout.header,
            &hdr_default,
        );
        let ftr = select(
            pn,
            &layout.footer_first,
            &ftr_first,
            &layout.footer_even,
            &ftr_even,
            &layout.footer,
            &ftr_default,
        );

        if let Some((hf, pre)) = hdr {
            let (mut items, h) = match pre {
                Some((items, h)) => (items.clone(), *h),
                // Contains page fields — lay out fresh for this page.
                None => layout_blocks_reflow(
                    resources,
                    &hf.blocks,
                    catalog,
                    content_width,
                    display_scale,
                    Some(ctx),
                ),
            };
            for item in &mut items {
                item.translate(left_margin, hdr_margin_y);
            }
            page.header_items = items;
            page.header_height = h;
        }

        if let Some((hf, pre)) = ftr {
            let (mut items, h) = match pre {
                Some((items, h)) => (items.clone(), *h),
                None => layout_blocks_reflow(
                    resources,
                    &hf.blocks,
                    catalog,
                    content_width,
                    display_scale,
                    Some(ctx),
                ),
            };
            let footer_y = page_h - ftr_margin - h;
            for item in &mut items {
                item.translate(left_margin, footer_y);
            }
            page.footer_items = items;
            page.footer_height = h;
        }
    }
}

// ── Miscellaneous block renderers ─────────────────────────────────────────────

fn flow_hrule(state: &mut FlowState) {
    const RULE_HEIGHT: f32 = 1.0;
    const RULE_SPACING: f32 = 6.0;
    state
        .current_items
        .push(PositionedItem::HorizontalRule(PositionedRect {
            rect: LayoutRect::new(0.0, state.cursor_y, state.content_width, RULE_HEIGHT),
            color: LayoutColor::BLACK,
        }));
    state.cursor_y += RULE_HEIGHT + RULE_SPACING;
}

// ── Footnote rendering ────────────────────────────────────────────────────────

/// Render all accumulated footnotes at the end of the section.
///
/// Places a 1/3-width separator rule followed by each note body. The note
/// reference mark (e.g. "¹") is prepended to the first block of each note.
/// End-of-section placement is used for v0.1; end-of-page is deferred.
fn flow_footnotes(state: &mut FlowState) {
    if state.pending_footnotes.is_empty() {
        return;
    }
    let notes = std::mem::take(&mut state.pending_footnotes);

    // Separator: 1/3-width, 0.5 pt tall, 4 pt spacing above and below.
    const SEP_HEIGHT: f32 = 0.5;
    const SEP_GAP: f32 = 4.0;
    let sep_w = state.content_width / 3.0;
    state.cursor_y += SEP_GAP;
    state
        .current_items
        .push(PositionedItem::HorizontalRule(PositionedRect {
            rect: LayoutRect::new(0.0, state.cursor_y, sep_w, SEP_HEIGHT),
            color: LayoutColor::BLACK,
        }));
    state.cursor_y += SEP_HEIGHT + SEP_GAP;

    for note in notes {
        let mark = format!("{} ", &footnote_mark(note.number));
        let mut first = true;
        for (body_block, block) in note.blocks.iter().enumerate() {
            // Tag body paragraph(s) so a click into the footnote resolves to the
            // live note-body container.
            state.nested_editing = Some(editing::NestedEditing::note(
                note.owner_block_index,
                note.note_in_block,
                body_block,
            ));
            if first {
                first = false;
                if let Block::StyledPara(p) = block {
                    let mut p = p.clone();
                    p.inlines.insert(0, Inline::Str(mark.clone()));
                    flow_paragraph(state, &p, 0);
                    continue;
                }
            }
            flow_block(state, block, 0);
        }
    }
    state.nested_editing = None;
}

/// Return the Unicode superscript mark for note number `n`.
fn footnote_mark(n: u32) -> String {
    match n {
        1 => "\u{00B9}".to_string(),
        2 => "\u{00B2}".to_string(),
        3 => "\u{00B3}".to_string(),
        4 => "\u{2074}".to_string(),
        5 => "\u{2075}".to_string(),
        6 => "\u{2076}".to_string(),
        7 => "\u{2077}".to_string(),
        8 => "\u{2078}".to_string(),
        9 => "\u{2079}".to_string(),
        _ => format!("[{n}]"),
    }
}

// ── Paragraph synthesisers ────────────────────────────────────────────────────

pub(super) fn synthesize_plain_para(inlines: &[Inline]) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

pub(super) fn synthesize_heading_para(
    level: u8,
    attr: &NodeAttr,
    inlines: &[Inline],
) -> StyledParagraph {
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
    // Prefer the style name carried in NodeAttr (set by the ODF mapper from
    // text:style-name so the catalog can resolve ODF heading properties like
    // font-size and bold). Fall back to the canonical OOXML/internal names.
    let style_id: StyleId = attr
        .kv
        .iter()
        .find(|(k, _)| k == "style")
        .map(|(_, v)| StyleId::new(v.as_str()))
        .unwrap_or_else(|| {
            let hardcoded = match level {
                1 => "Heading1",
                2 => "Heading2",
                3 => "Heading3",
                4 => "Heading4",
                5 => "Heading5",
                _ => "Heading6",
            };
            StyleId::new(hardcoded)
        });
    let direct_alignment =
        attr.kv
            .iter()
            .find(|(k, _)| k == "jc")
            .and_then(|(_, v)| match v.as_str() {
                "center" => Some(ParagraphAlignment::Center),
                "right" => Some(ParagraphAlignment::Right),
                "justify" => Some(ParagraphAlignment::Justify),
                _ => None,
            });
    let direct_para_props = direct_alignment.map(|align| {
        Box::new(ParaProps {
            alignment: Some(align),
            ..Default::default()
        })
    });
    StyledParagraph {
        style_id: Some(style_id),
        direct_para_props,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

// ── Table layout ─────────────────────────────────────────────────────────────

pub(super) fn get_items_max_x(items: &[PositionedItem]) -> f32 {
    let mut max_x = 0.0f32;
    for item in items {
        let x = match item {
            PositionedItem::GlyphRun(r) => {
                let mut run_max = r.origin.x;
                for g in &r.glyphs {
                    let right = r.origin.x + g.x + g.advance;
                    if right > run_max {
                        run_max = right;
                    }
                }
                run_max
            }
            PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => {
                r.rect.origin.x + r.rect.size.width
            }
            PositionedItem::BorderRect(r) => r.rect.origin.x + r.rect.size.width,
            PositionedItem::Image(r) => r.rect.origin.x + r.rect.size.width,
            PositionedItem::Decoration(d) => d.x + d.width,
            PositionedItem::ClippedGroup { clip_rect, items } => {
                let inner_max = get_items_max_x(items);
                inner_max.min(clip_rect.origin.x + clip_rect.size.width)
            }
            PositionedItem::RotatedGroup {
                origin,
                content_width,
                ..
            } => origin.x + content_width,
        };
        if x > max_x {
            max_x = x;
        }
    }
    max_x
}

fn flow_table(
    state: &mut FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
    idx: usize,
) {
    let col_widths = table_geom::resolve_column_widths(state, tbl);

    let mut rows = Vec::new();
    rows.extend(&tbl.head.rows);
    for body in &tbl.bodies {
        rows.extend(&body.head_rows);
        rows.extend(&body.body_rows);
    }
    rows.extend(&tbl.foot.rows);

    // Assign each cell its grid columns, accounting for columns covered by a
    // `row_span` (vMerge) cell from an earlier row (`cell_cols[row][cell] =
    // (col_start, col_end)`). Without it a cell whose leading column is occupied
    // by a vertical merge above is placed too far left — the TC-DOCX-005 bug.
    let cell_cols = table_geom::assign_cell_columns(&rows, col_widths.len());

    // Named style + `w:tblLook` → conditional/banding shading (under direct).
    let table_style = resolve_table_style(state.catalog, tbl.style_name());
    let look = table_look(tbl);
    let (grid_rows, grid_cols) = (rows.len(), col_widths.len());

    let row_heights = table_paint::measure_row_heights(state, &rows, &cell_cols, &col_widths, idx);

    // Pass 3: Place and flow cell blocks. `cell_flat` counts cells in the bridge's
    // flat `KEY_TABLE_CELLS` order so cell paragraphs get a matching `PathStep::Cell`.
    let mut cell_flat = 0usize;
    for (row_idx, row) in rows.iter().enumerate() {
        let row_max_h = row_heights[row_idx];

        if state.mode.is_paginated() {
            let remaining_h = state.page_content_height - state.cursor_y;
            if row_max_h > remaining_h && row_max_h <= state.page_content_height {
                // A whole row that fits in a band but not the remaining space
                // moves to the next column (or page).
                columns_impl::break_column(state);
            }
        }

        let original_row_page = state.page_number;
        let original_row_y_start = state.cursor_y;
        let table_indent = state.current_indent;

        let cell_starts = table_cells::flow_row_cells(
            state,
            row,
            row_idx,
            &cell_cols[row_idx],
            &col_widths,
            &row_heights,
            row_max_h,
            original_row_page,
            original_row_y_start,
            idx,
            &mut cell_flat,
        );

        let row_page_end = state.page_number;
        let row_y_end = if original_row_page == row_page_end {
            original_row_y_start + row_max_h
        } else {
            let first_h = (state.page_content_height - original_row_y_start).max(0.0);
            let intermediate_h =
                (row_page_end - original_row_page - 1) as f32 * state.page_content_height;
            (row_max_h - first_h - intermediate_h).max(0.0)
        };

        table_paint::emit_row_cell_decorations(
            state,
            row,
            row_idx,
            &cell_cols[row_idx],
            &col_widths,
            &row_heights,
            row_max_h,
            &cell_starts,
            table_indent,
            table_style,
            &look,
            grid_rows,
            grid_cols,
            original_row_page,
            original_row_y_start,
            row_page_end,
        );

        state.cursor_y = row_y_end;
    }
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
