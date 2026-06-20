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

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::style::list_style::ListId;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::result::{LayoutPage, PaginatedLayout};

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

/// A clean-page-top checkpoint: which page started, at which top-level block,
/// and the [`FlowCheckpoint`] needed to resume the flow there.
#[derive(Debug, Clone)]
pub struct PageStart {
    /// Index into `PaginatedLayout::pages` of the page that starts here.
    pub page_index: usize,
    /// Index of the top-level section-0 block this page begins flowing.
    pub block_index: usize,
    /// Resumable flow state at this page top.
    pub(crate) checkpoint: FlowCheckpoint,
}

/// Returns the index of the first block that differs between `old` and `new`,
/// or `None` when the two block slices are equal. Requires equal length (a
/// length change is a structural edit the incremental path does not handle).
pub(crate) fn first_changed_block(old: &[Block], new: &[Block]) -> Option<usize> {
    if old.len() != new.len() {
        return Some(old.len().min(new.len()));
    }
    old.iter().zip(new.iter()).position(|(a, b)| a != b)
}

/// Returns `true` when `old[from..]` and `new[from..]` are element-wise equal —
/// i.e. every block from `from` onward is unchanged. Used to license suffix
/// reuse: equal trailing blocks + an equal checkpoint ⇒ identical trailing pages.
pub(crate) fn blocks_equal_from(old: &[Block], new: &[Block], from: usize) -> bool {
    old.len() == new.len() && old[from..] == new[from..]
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
    /// Clean-page-top checkpoints, in increasing page/block order.
    pub checkpoints: Vec<PageStart>,
    /// Whether the document contains any footnote/endnote. Footnotes render at
    /// section end, so a content change can renumber/repaginate the tail —
    /// incremental reuse is disabled when this is set.
    pub has_footnotes: bool,
}

/// `true` if any inline in `inlines` (recursively) is a footnote/endnote.
fn inlines_have_note(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Note(..) => true,
        Inline::Strong(c)
        | Inline::Emph(c)
        | Inline::Underline(c)
        | Inline::Strikeout(c)
        | Inline::Superscript(c)
        | Inline::Subscript(c)
        | Inline::SmallCaps(c)
        | Inline::Quoted(_, c)
        | Inline::Span(_, c)
        | Inline::Cite(_, c) => inlines_have_note(c),
        Inline::Link(_, c, _) => inlines_have_note(c),
        Inline::StyledRun(run) => inlines_have_note(&run.content),
        _ => false,
    })
}

/// `true` if `block` (recursively) contains a footnote/endnote.
pub(crate) fn block_has_note(block: &Block) -> bool {
    use loki_doc_model::content::table::core::Table;
    fn table_has_note(t: &Table) -> bool {
        t.head
            .rows
            .iter()
            .chain(t.foot.rows.iter())
            .chain(
                t.bodies
                    .iter()
                    .flat_map(|b| b.head_rows.iter().chain(b.body_rows.iter())),
            )
            .any(|row| {
                row.cells
                    .iter()
                    .any(|c| c.blocks.iter().any(block_has_note))
            })
    }
    match block {
        Block::Para(i) | Block::Plain(i) | Block::Heading(_, _, i) => inlines_have_note(i),
        Block::StyledPara(p) => inlines_have_note(&p.inlines),
        Block::LineBlock(lines) => lines.iter().any(|l| inlines_have_note(l)),
        Block::BlockQuote(ch) | Block::Div(_, ch) | Block::Figure(_, _, ch) => {
            ch.iter().any(block_has_note)
        }
        Block::OrderedList(_, items) | Block::BulletList(items) => {
            items.iter().flatten().any(block_has_note)
        }
        Block::Table(t) => table_has_note(t),
        _ => false,
    }
}

/// `true` if any block in `doc` contains a footnote/endnote. Computed once on
/// the full-layout path (where the cost is already O(document)).
pub fn document_has_notes(doc: &Document) -> bool {
    doc.sections
        .iter()
        .any(|s| s.blocks.iter().any(block_has_note))
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
        || doc.sections.len() != 1
        || prev_doc.sections.len() != 1
        || prev_reuse.has_footnotes
        || prev_reuse.checkpoints.is_empty()
    {
        return None;
    }

    let new_blocks = &doc.sections[0].blocks;
    let old_blocks = &prev_doc.sections[0].blocks;
    if new_blocks.len() != old_blocks.len() {
        return None; // block insert/delete/move — structural
    }

    let Some(c) = first_changed_block(old_blocks, new_blocks) else {
        // No content change — reuse the previous layout verbatim.
        return Some((prev_layout.clone(), prev_reuse.clone()));
    };
    if block_has_note(&new_blocks[c]) {
        return None; // the edit introduced a footnote
    }

    // ── Prefix: last clean page top at or before the changed block ──
    let pp = prev_reuse
        .checkpoints
        .iter()
        .rfind(|cp| cp.block_index <= c)?;
    let prefix_pages = pp.page_index;
    let mut pages: Vec<LayoutPage> = prev_layout.pages[..prefix_pages].to_vec();
    let mut new_checkpoints: Vec<PageStart> = prev_reuse
        .checkpoints
        .iter()
        .take_while(|cp| cp.page_index < prefix_pages)
        .cloned()
        .collect();

    // ── Re-flow from the prefix boundary, resyncing against old checkpoints ──
    let mut splice_from: Option<usize> = None;
    let resumed = crate::flow::flow_section_resume(
        resources,
        &doc.sections[0],
        &doc.styles,
        display_scale,
        options,
        pp.block_index,
        &pp.checkpoint,
        |b, s| {
            if let Some(old) = prev_reuse
                .checkpoints
                .iter()
                .find(|cp| cp.block_index == b && &cp.checkpoint == s)
                && blocks_equal_from(old_blocks, new_blocks, b)
            {
                splice_from = Some(old.page_index);
                return true;
            }
            false
        },
    );

    pages.extend(resumed.pages);
    for cp in resumed.checkpoints {
        new_checkpoints.push(PageStart {
            page_index: cp.page_index + prefix_pages,
            block_index: cp.block_index,
            checkpoint: cp.checkpoint,
        });
    }

    // ── Suffix splice when the flow resynchronised ──
    if let Some(splice) = splice_from {
        pages.extend(prev_layout.pages[splice..].iter().cloned());
        new_checkpoints.extend(
            prev_reuse
                .checkpoints
                .iter()
                .filter(|cp| cp.page_index >= splice)
                .cloned(),
        );
    }

    // ── Renumber and re-assign headers/footers (NUMPAGES depends on count) ──
    for (idx, page) in pages.iter_mut().enumerate() {
        page.page_number = idx + 1;
    }
    let total = pages.len() as u32;
    crate::flow::assign_headers_footers(
        &mut pages,
        &doc.sections[0].layout,
        resources,
        &doc.styles,
        display_scale,
        total,
    );

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
