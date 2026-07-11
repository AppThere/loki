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
//! Only `flow_section` (standalone paginated sections) routes through here.
//! Multi-page sections, `continuous` section groups, and sections carrying
//! footnotes keep the fill-first behaviour (capping the content height would
//! misplace footnotes and page-bottom content); those remain a documented
//! limitation. See `docs/fidelity-status.md` (Multi-column Sections).
//!
//! TODO(column-balance-multipage): balance the *last page only* of a multi-page
//! or continuous multi-column section (isolate its tail content via the flow
//! checkpoints and re-flow it with a capped column height), and handle
//! footnote-bearing sections without displacing the notes.

use loki_doc_model::StyleCatalog;
use loki_doc_model::content::annotation::Comment;
use loki_doc_model::layout::section::Section;

use super::{FlowOutput, flow_footnotes, new_flow_state, run_paginated_loop};
use crate::LayoutOptions;
use crate::font::FontResources;
use crate::mode::LayoutMode;
use crate::resolve::pts_to_f32;

/// Maximum binary-search iterations when locating the balanced column height.
/// ~16 halvings resolve a full page height to well under a point.
const MAX_ITERS: u32 = 16;

/// Flows a paginated section, balancing the columns when it is a multi-column
/// section that fits on a single page and carries no footnotes.
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
    let (natural, pages, has_notes) = run_capped(resources, &ctx, None);
    if !is_multicolumn(section) || pages != 1 || has_notes {
        return natural;
    }
    let full_h = full_content_height(section);
    let Some(cap) = find_balanced_height(resources, &ctx, full_h) else {
        return natural;
    };
    let (balanced, bpages, _) = run_capped(resources, &ctx, Some(cap));
    // Guard: only adopt the balanced layout if it still fits on one page.
    if bpages == 1 { balanced } else { natural }
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

/// Runs one full paginated flow, optionally capping the per-page content height
/// (the column-break threshold). Returns the output, the page count, and
/// whether any footnote was emitted.
fn run_capped(
    resources: &mut FontResources,
    ctx: &Ctx<'_>,
    cap: Option<f32>,
) -> (FlowOutput, usize, bool) {
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
    run_paginated_loop(&mut state, &ctx.section.blocks, 0, 0, |_, _| false);
    flow_footnotes(&mut state);
    let has_notes = state.note_counter > 0;
    super::finish_page(&mut state);
    let pages = state.pages.len();
    (
        FlowOutput::Pages {
            pages: state.pages,
            checkpoints: state.checkpoints,
            warnings: state.warnings,
        },
        pages,
        has_notes,
    )
}

/// Binary-searches the smallest content height at which the section still fits
/// on a single page. Feasibility is monotonic (a taller cap needs no more
/// columns), so the threshold packs the columns as evenly as possible. Returns
/// `None` when the page is degenerate (zero height).
fn find_balanced_height(resources: &mut FontResources, ctx: &Ctx<'_>, full_h: f32) -> Option<f32> {
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
        let (_, pages, _) = run_capped(resources, ctx, Some(mid));
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
