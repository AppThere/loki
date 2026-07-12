// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Incremental paginated relayout.
//!
//! See `docs/incremental-layout.md` for the full design. In short: a full
//! paginated layout records a [`PageStart`] checkpoint at every *clean page top*
//! (a page boundary that falls between top-level blocks, with the content-area
//! cursor at 0 and no items yet placed). Given the previous layout, its
//! checkpoints, and the previous document, [`relayout_paginated_incremental`]
//! reuses the unchanged prefix of pages, re-flows from the changed block, and
//! splices the unchanged suffix back when the flow state resynchronises — so a
//! height-preserving single-block edit costs O(one page) instead of O(document).
//!
//! The driver only returns `Some` when it can prove the result equals a full
//! layout; otherwise it returns `None` and the caller runs `layout_document`.
//! The `incremental == full` property test in `incremental_tests.rs` is the
//! correctness gate.

use std::collections::HashMap;
use std::sync::Arc;

use loki_doc_model::document::Document;
use loki_doc_model::style::list_style::ListId;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::result::{LayoutPage, PaginatedLayout};

#[path = "incremental_diff.rs"]
mod diff;
use diff::{blocks_equal_from, common_prefix_len, common_suffix_len, section_page_start};
#[path = "incremental_notes.rs"]
mod notes;
use notes::block_has_note;
pub use notes::document_has_notes;

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
}

/// Attempts an incremental paginated relayout of `doc` given the previous
/// document, its layout, and its [`PaginatedReuse`] metadata.
///
/// Returns `Some((layout, reuse))` only when the result is provably identical to
/// a full `layout_paginated_full`; otherwise returns `None` and the caller must
/// run the full layout. See `docs/incremental-layout.md` and the `incremental ==
/// full` property test.
#[allow(clippy::too_many_arguments)]
pub fn relayout_paginated_incremental(
    resources: &mut FontResources,
    doc: &Document,
    prev_doc: &Document,
    prev_layout: &PaginatedLayout,
    prev_reuse: &PaginatedReuse,
    display_scale: f32,
    options: &LayoutOptions,
) -> Option<(PaginatedLayout, PaginatedReuse)> {
    // ── Eligibility (mirrors loro_bridge::IncrementalReader's fast-path) ──
    if !options.preserve_for_editing
        || prev_reuse.has_footnotes
        || prev_reuse.checkpoints.is_empty()
        || doc.sections.len() != prev_doc.sections.len()
    {
        return None;
    }

    // Exactly one section's blocks may differ, and its page layout (margins,
    // headers, page size) must be unchanged; any other difference is structural.
    let mut changed = None;
    for i in 0..doc.sections.len() {
        if doc.sections[i] != prev_doc.sections[i] {
            if changed.is_some() || doc.sections[i].layout != prev_doc.sections[i].layout {
                return None;
            }
            changed = Some(i);
        }
    }
    let Some(sc) = changed else {
        // Nothing changed — reuse the previous layout verbatim.
        return Some((prev_layout.clone(), prev_reuse.clone()));
    };

    // Multi-column sections are column-balanced by the full flow, not the resume
    // path; an edit to one falls back to a full relayout.
    if doc.sections[sc]
        .layout
        .columns
        .as_ref()
        .is_some_and(|c| c.count >= 2)
    {
        return None;
    }

    let new_blocks = &doc.sections[sc].blocks;
    let old_blocks = &prev_doc.sections[sc].blocks;
    // First changed block (works across a block insert/delete). The section
    // differs and its layout is unchanged, so the blocks genuinely differ.
    let c = common_prefix_len(old_blocks, new_blocks);
    if c == old_blocks.len() && c == new_blocks.len() {
        // Blocks are identical (the section differed only in non-block data);
        // the layout is unchanged, so reuse it verbatim.
        return Some((prev_layout.clone(), prev_reuse.clone()));
    }
    let suffix = common_suffix_len(old_blocks, new_blocks, c);
    // A footnote introduced anywhere in the changed region disables reuse (the
    // previous layout is already gated on containing no notes).
    if new_blocks[c..new_blocks.len() - suffix]
        .iter()
        .any(block_has_note)
    {
        return None;
    }
    // Same block count ⇒ in-section suffix resync is sound (block indices align).
    // A count change (insert/delete) shifts later block indices, so resync is
    // disabled and section `sc` is re-flowed to its end; later whole sections are
    // still reusable when `sc`'s page count is unchanged.
    let allow_resync = old_blocks.len() == new_blocks.len();

    let total_pages = prev_layout.pages.len();
    let sc_start = section_page_start(&prev_reuse.checkpoints, sc)?;
    let sc_old_end = section_page_start(&prev_reuse.checkpoints, sc + 1).unwrap_or(total_pages);
    let is_last_section = sc + 1 == doc.sections.len();

    // ── Prefix: last clean page top in section `sc` at or before block `c` ──
    let pp = prev_reuse
        .checkpoints
        .iter()
        .rfind(|cp| cp.section_index == sc && cp.block_index <= c)?;
    let prefix_pages = pp.page_index;
    let mut new_checkpoints: Vec<PageStart> = prev_reuse
        .checkpoints
        .iter()
        .take_while(|cp| cp.page_index < prefix_pages)
        .cloned()
        .collect();

    // ── Re-flow section `sc` from the prefix boundary, resyncing against the old
    // section-`sc` checkpoints. A resync splices the global page suffix (this
    // section's tail *and every later section*), which are all unchanged. ──
    let mut splice_from: Option<usize> = None;
    let resumed = crate::flow::flow_section_resume(
        resources,
        &doc.sections[sc],
        &doc.styles,
        display_scale,
        options,
        pp.block_index,
        &pp.checkpoint,
        |b, s| {
            if allow_resync
                && let Some(old) = prev_reuse
                    .checkpoints
                    .iter()
                    .find(|cp| cp.section_index == sc && cp.block_index == b && &cp.checkpoint == s)
                && blocks_equal_from(old_blocks, new_blocks, b)
            {
                splice_from = Some(old.page_index);
                return true;
            }
            false
        },
    );

    // Re-flowed ("middle") pages carry section-local numbers from the resumed
    // flow; lift them to document-global by the section's page start (0 for a
    // single-section document). Reused pages keep their numbers — no `make_mut`.
    let mut middle = resumed.pages;
    for page in &mut middle {
        page.page_number += sc_start;
    }
    for cp in resumed.checkpoints {
        new_checkpoints.push(PageStart {
            page_index: cp.page_index + prefix_pages,
            section_index: sc,
            block_index: cp.block_index,
            checkpoint: cp.checkpoint,
        });
    }

    // ── Decide the reusable suffix and the new total page count ──
    let sc_new_end = prefix_pages + middle.len();
    let suffix_start = match splice_from {
        // Resynced inside `sc`: everything from here to the document end is
        // identical (section tail + later sections). Count is unchanged.
        Some(splice) => Some(splice),
        // `sc` re-flowed to its end without a resync. Later pages are reusable
        // only if the section's page count is unchanged (so they stay aligned).
        None if sc_new_end == sc_old_end && !is_last_section => Some(sc_old_end),
        None if is_last_section => None, // last section: a count change is fine
        None => return None,             // count change with later sections → full
    };
    let new_total = suffix_start.map_or(sc_new_end, |s| sc_new_end + (total_pages - s));
    if let Some(splice) = suffix_start {
        new_checkpoints.extend(
            prev_reuse
                .checkpoints
                .iter()
                .filter(|cp| cp.page_index >= splice)
                .cloned(),
        );
    }

    // ── Headers/footers. Reused pages keep theirs (valid because their page
    // numbers are unchanged); assign only the fresh middle pages. The exception
    // is a page-count change combined with a header that references the count —
    // then reused NUMPAGES is stale, so re-run the full per-section pass. ──
    if new_total != total_pages
        && doc
            .sections
            .iter()
            .any(|s| crate::flow::page_layout_has_page_fields(&s.layout))
    {
        return None;
    }
    crate::flow::assign_headers_footers(
        &mut middle,
        &doc.sections[sc].layout,
        resources,
        &doc.styles,
        display_scale,
        new_total as u32,
    );

    // ── Assemble: reused prefix (Arc bump) + fresh middle + reused suffix ──
    let mut pages: Vec<Arc<LayoutPage>> = Vec::with_capacity(new_total);
    pages.extend(prev_layout.pages[..prefix_pages].iter().cloned());
    pages.extend(middle.into_iter().map(Arc::new));
    if let Some(splice) = suffix_start {
        pages.extend(prev_layout.pages[splice..].iter().cloned());
    }

    Some((
        PaginatedLayout {
            page_size: prev_layout.page_size,
            pages,
        },
        PaginatedReuse {
            checkpoints: new_checkpoints,
            has_footnotes: false,
        },
    ))
}

#[cfg(test)]
#[path = "incremental_tests.rs"]
mod tests;
