// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Column-height balancing for paginated multi-column sections.
//!
//! Word balances the columns of a multi-column section's **last** page so they
//! end at roughly equal heights, rather than filling column 1 to the page
//! bottom before starting column 2. This module implements that for the common
//! case: a multi-column section whose content fits on a single page (which *is*
//! its last page). It re-flows the section with the per-page content height
//! capped to the smallest value that still fits every column on one page — the
//! tightest, evenly-filled packing — found by a bounded binary search.
//!
//! A **multi-page** section balances its *last page only*: the natural flow
//! records a *tail candidate* (the block that started the newest page, plus a
//! resume snapshot — see `FlowState::tail_candidate`), the tail is re-flowed
//! uncapped once to **verify** the candidate reproduces the natural last page
//! (a page starting mid-paragraph fails this and keeps fill-first), then the
//! verified tail is re-flowed with the balanced cap and spliced over the
//! natural last page — earlier pages are untouched. `continuous` section
//! groups and sections carrying footnotes keep the fill-first behaviour
//! (capping the content height would misplace footnotes, and a group tail can
//! start mid-page inside another section); those remain documented
//! limitations. See `docs/fidelity-status.md` (Multi-column Sections).

use loki_doc_model::StyleCatalog;
use loki_doc_model::content::annotation::Comment;
use loki_doc_model::layout::section::Section;

use super::{FlowOutput, new_flow_state, run_paginated_loop};
use crate::LayoutOptions;
use crate::font::FontResources;
use crate::incremental::FlowCheckpoint;
use crate::mode::LayoutMode;
use crate::resolve::pts_to_f32;

/// Maximum binary-search iterations when locating the balanced column height.
/// ~16 halvings resolve a full page height to well under a point.
const MAX_ITERS: u32 = 16;

/// Flows a paginated section, balancing the columns when it is a multi-column
/// section without footnotes: the whole section when it fits on one page,
/// otherwise the last page only (resumed from its clean-top checkpoint).
pub(super) fn flow_paginated_balanced(
    resources: &mut FontResources,
    section: &Section,
    catalog: &StyleCatalog,
    mode: &LayoutMode,
    display_scale: f32,
    options: &LayoutOptions,
    comments: &[Comment],
) -> FlowOutput {
    let ctx = Ctx {
        section,
        catalog,
        mode,
        display_scale,
        options,
        comments,
    };
    let (natural, pages, has_notes, candidate) = run_capped(resources, &ctx, None, None);
    if !is_multicolumn(section) || has_notes {
        return natural;
    }
    if pages > 1 {
        return balance_last_page(resources, &ctx, natural, candidate);
    }
    let full_h = full_content_height(section);
    let Some(cap) = find_balanced_height(resources, &ctx, full_h, None) else {
        return natural;
    };
    let (balanced, bpages, _, _) = run_capped(resources, &ctx, Some(cap), None);
    // Guard: only adopt the balanced layout if it still fits on one page.
    if bpages == 1 { balanced } else { natural }
}

/// Re-flows the tail of a multi-page section (from the natural run's tail
/// candidate) with the balanced column cap and splices the result over the
/// natural last page. Falls back to `natural` when there is no candidate for
/// the last page, when the uncapped tail replay does not reproduce the natural
/// last page (it starts mid-block), or when the balanced tail no longer fits
/// one page.
fn balance_last_page(
    resources: &mut FontResources,
    ctx: &Ctx<'_>,
    natural: FlowOutput,
    candidate: Option<crate::incremental::PageStart>,
) -> FlowOutput {
    let FlowOutput::Pages {
        mut pages,
        checkpoints,
        warnings,
    } = natural
    else {
        return natural;
    };
    let last = pages.len() - 1;
    if let Some(cand) = candidate.filter(|c| c.page_index == last) {
        let tail = Some((cand.block_index, &cand.checkpoint));
        // Verify: the uncapped tail replay must reproduce the natural last
        // page (same single page, same glyph-run and item counts) — otherwise
        // the page starts mid-block and cannot be resumed from a block seed.
        let (probe, ppages, _, _) = run_capped(resources, ctx, None, tail);
        let reproduces = ppages == 1
            && matches!(&probe, FlowOutput::Pages { pages: pp, .. }
                if pp.first().is_some_and(|p| pages_match(p, &pages[last])));
        if reproduces {
            let full_h = full_content_height(ctx.section);
            if let Some(cap) = find_balanced_height(resources, ctx, full_h, tail) {
                let (balanced, bpages, _, _) = run_capped(resources, ctx, Some(cap), tail);
                if bpages == 1
                    && let FlowOutput::Pages {
                        pages: mut tail_pages,
                        ..
                    } = balanced
                    && let Some(balanced_last) = tail_pages.pop()
                {
                    pages[last] = balanced_last;
                }
            }
        }
    }
    FlowOutput::Pages {
        pages,
        checkpoints,
        warnings,
    }
}

/// Whether two pages carry the same content by cheap structural digest: equal
/// item counts and equal (recursive) glyph-run counts. Floats are not compared
/// — identical inputs produce identical counts, which is all the verification
/// needs to reject a mid-block tail (it re-places the whole block, changing
/// both counts).
fn pages_match(a: &crate::result::LayoutPage, b: &crate::result::LayoutPage) -> bool {
    a.content_items.len() == b.content_items.len()
        && count_glyph_runs(&a.content_items) == count_glyph_runs(&b.content_items)
}

/// Recursively counts glyph runs, descending into clipped groups.
fn count_glyph_runs(items: &[crate::items::PositionedItem]) -> usize {
    items
        .iter()
        .map(|i| match i {
            crate::items::PositionedItem::GlyphRun(_) => 1,
            crate::items::PositionedItem::ClippedGroup { items, .. } => count_glyph_runs(items),
            _ => 0,
        })
        .sum()
}

/// The unchanging arguments threaded through the repeated flow probes.
struct Ctx<'a> {
    section: &'a Section,
    catalog: &'a StyleCatalog,
    mode: &'a LayoutMode,
    display_scale: f32,
    options: &'a LayoutOptions,
    comments: &'a [Comment],
}

/// Runs one paginated flow — the whole section, or its tail when
/// `tail = Some((start_block, seed))` resumes from a clean-page-top checkpoint
/// (the [`super::flow_section_resume`] seeding) — optionally capping the
/// per-page content height (the column-break threshold). Returns the output,
/// the page count, and whether any footnote was emitted.
fn run_capped(
    resources: &mut FontResources,
    ctx: &Ctx<'_>,
    cap: Option<f32>,
    tail: Option<(usize, &FlowCheckpoint)>,
) -> (
    FlowOutput,
    usize,
    bool,
    Option<crate::incremental::PageStart>,
) {
    let mut state = new_flow_state(
        resources,
        ctx.section,
        ctx.catalog,
        ctx.mode,
        ctx.display_scale,
        ctx.options,
        ctx.comments,
    );
    if let Some(h) = cap {
        state.page_content_height = h.clamp(1.0, state.page_content_height);
    }
    let mut start = 0;
    if let Some((start_block, seed)) = tail {
        state.page_number = seed.page_number;
        state.list_counters = seed.list_counters.clone();
        state.prev_list_id = seed.prev_list_id.clone();
        state.note_counter = seed.note_counter;
        state.current_indent = seed.current_indent;
        start = start_block;
    }
    run_paginated_loop(&mut state, &ctx.section.blocks, start, 0, |_, _| false);
    let has_notes = state.note_counter > 0;
    // `finish_page` lays out the final page's footnote band (per-page placement).
    super::finish_page(&mut state);
    let pages = state.pages.len();
    let candidate = state.tail_candidate.take();
    (
        FlowOutput::Pages {
            pages: state.pages,
            checkpoints: state.checkpoints,
            warnings: state.warnings,
        },
        pages,
        has_notes,
        candidate,
    )
}

/// Binary-searches the smallest content height at which the flowed content
/// (whole section, or the `tail` from the last page's checkpoint) still fits
/// on a single page. Feasibility is monotonic (a taller cap needs no more
/// columns), so the threshold packs the columns as evenly as possible. Returns
/// `None` when the page is degenerate (zero height).
fn find_balanced_height(
    resources: &mut FontResources,
    ctx: &Ctx<'_>,
    full_h: f32,
    tail: Option<(usize, &FlowCheckpoint)>,
) -> Option<f32> {
    if full_h <= 1.0 {
        return None;
    }
    // `lo` is the infeasible side (too short → overflows to a 2nd page), `hi`
    // the feasible side (the natural full height fits on one page).
    let mut lo = 0.0f32;
    let mut hi = full_h;
    for _ in 0..MAX_ITERS {
        if hi - lo < 0.5 {
            break;
        }
        let mid = 0.5 * (lo + hi);
        let (_, pages, _, _) = run_capped(resources, ctx, Some(mid), tail);
        if pages == 1 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

/// Whether the section requests two or more columns.
fn is_multicolumn(section: &Section) -> bool {
    section
        .layout
        .columns
        .as_ref()
        .is_some_and(|c| c.count >= 2)
}

/// The full per-page content height (page height minus vertical margins), the
/// same value `new_flow_state` derives — the upper bound of the search.
fn full_content_height(section: &Section) -> f32 {
    let pl = &section.layout;
    let page_h = pts_to_f32(pl.page_size.height);
    let vmargin = pts_to_f32(pl.margins.top) + pts_to_f32(pl.margins.bottom);
    (page_h - vmargin).max(0.0)
}
