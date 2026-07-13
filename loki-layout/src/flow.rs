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

use loki_doc_model::StyleCatalog;
use loki_doc_model::style::list_style::ListId;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutSize};
use crate::incremental::{FlowCheckpoint, PageStart};
use crate::items::PositionedItem;
use crate::mode::LayoutMode;
use crate::resolve::CollectedNote;
use crate::result::{LayoutPage, PageParagraphData};

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

// ── Flow construction, entry points, and block dispatch (submodules) ───────────

#[path = "flow_dispatch.rs"]
mod dispatch;
#[path = "flow_entry.rs"]
mod entry;
#[path = "flow_run.rs"]
mod run;

pub(super) use dispatch::{finish_page, flow_block};
use entry::begin_continuous_section;
pub use entry::flow_section;
pub(crate) use entry::flow_section_resume;
use run::{new_flow_state, run_paginated_loop};

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
