// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The types incremental paginated relayout resumes from and reports with.
//!
//! Split from `incremental.rs` when the `reflowed_pages` counter pushed that file
//! over the 300-line ceiling. The cut is cohesive: everything here is *state
//! carried between layout passes*, while the parent is the algorithm that consumes
//! it.

use std::collections::HashMap;

use loki_doc_model::style::list_style::ListId;

use crate::result::LayoutPage;

/// Resumable flow state captured at a clean page top.
///
/// At a clean page top the content cursor is 0 and the item/paragraph
/// accumulators are empty, so the only state that carries forward is the page
/// number, the list counters, and the note counter. Equality of two checkpoints
/// (plus equal trailing blocks) means the pages they produce are identical —
/// this is what licenses suffix reuse.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowCheckpoint {
    /// 1-indexed page number the resumed page will carry.
    pub(crate) page_number: usize,
    /// Per-list counter arrays (see `flow::FlowState::list_counters`).
    pub(crate) list_counters: HashMap<ListId, [u32; 9]>,
    /// Most recently placed list id (drives new-list counter resets).
    pub(crate) prev_list_id: Option<ListId>,
    /// Section-wide footnote/endnote counter.
    pub(crate) note_counter: u32,
    /// Accumulated horizontal indent (0 at the top level; kept for completeness).
    pub(crate) current_indent: f32,
}

/// A clean-page-top checkpoint: which page started, in which section, at which
/// (section-local) block, and the [`FlowCheckpoint`] needed to resume there.
#[derive(Debug, Clone)]
pub struct PageStart {
    /// Index into `PaginatedLayout::pages` (document-global) of the page here.
    pub page_index: usize,
    /// Index of the document section this page belongs to.
    pub section_index: usize,
    /// Index of the top-level block within its section that this page begins.
    pub block_index: usize,
    /// Resumable flow state at this page top (page number is section-local).
    pub(crate) checkpoint: FlowCheckpoint,
}

/// Pages produced by resuming a paginated flow; see [`crate::flow::flow_section_resume`].
pub(crate) struct ResumedFlow {
    pub(crate) pages: Vec<LayoutPage>,
    pub(crate) checkpoints: Vec<PageStart>,
}

/// Reuse metadata produced alongside a full paginated layout, stored by the
/// editor so the next edit can attempt [`relayout_paginated_incremental`].
#[derive(Debug, Clone)]
pub struct PaginatedReuse {
    /// Clean-page-top checkpoints across all sections, in increasing page order.
    pub checkpoints: Vec<PageStart>,
    /// Whether the document contains any footnote/endnote. Footnotes render at
    /// section end, so a content change can renumber/repaginate the tail —
    /// incremental reuse is disabled when this is set.
    pub has_footnotes: bool,
    /// How many pages this relayout actually re-flowed, as opposed to reusing.
    /// Zero after a full layout, which reuses nothing by definition.
    ///
    /// # Why elapsed time alone cannot judge a reuse result
    ///
    /// A mid-document edit that takes 5 ms is ambiguous: resync may have failed to
    /// stop, re-flowing to the end of the document, **or** resync may be working
    /// perfectly on a document whose pages are all tightly packed, where the
    /// pagination cascade legitimately runs a long way. Those are opposite verdicts
    /// and identical stopwatches.
    ///
    /// With this count they separate immediately — 2 pages at 5 ms is a per-page
    /// cost problem, 150 pages at 5 ms is the mechanism working on a hard document.
    /// Same move as counting reduced tiles instead of judging blur (Spec 08 R5a).
    pub reflowed_pages: usize,
}
