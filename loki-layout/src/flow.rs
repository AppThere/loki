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
#[path = "flow_line_numbers.rs"]
mod line_numbers;
#[path = "flow_page_fields.rs"]
mod page_fields;
#[path = "flow_para_between.rs"]
mod para_between;
#[path = "flow_para.rs"]
mod para_impl;
#[path = "flow_table_autofit.rs"]
mod table_autofit;
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
#[path = "flow_textbox.rs"]
mod textbox_impl;

pub use group::flow_section_group;
pub(crate) use headers::{PagePosition, assign_headers_footers, layout_blocks_reflow};
pub(crate) use page_fields::page_layout_has_page_fields;
use tail::{flow_footnotes, flow_hrule, get_items_max_x};
// Public for ADR-0017's DOM reflow view: paragraph synthesis and the list
// marker/indent rules, so neither path states either twice (see their defs).
pub use dispatch::{NESTED_INDENT_PT, list_marker, synthesize_list_item_para};
pub use tail::synthesize::{synthesize_heading_para, synthesize_plain_para};

use loki_doc_model::style::list_style::ListId;

use crate::incremental::{FlowCheckpoint, PageStart};
use crate::items::PositionedItem;
use crate::result::LayoutPage;

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

#[path = "flow_state.rs"]
mod state;
pub(super) use state::FlowState;

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
