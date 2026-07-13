// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Public section-flow entry points, split out of `flow.rs` for the 300-line
//! ceiling: `flow_section` (mode-dispatched full flow), `flow_section_resume`
//! (incremental relayout from a checkpoint), and `begin_continuous_section`
//! (mid-page column-layout transition). All are re-exported from `flow.rs`.

use loki_doc_model::{Section, StyleCatalog};

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::incremental::FlowCheckpoint;
use crate::mode::LayoutMode;

use super::{
    FlowOutput, FlowState, balance, columns_impl, finish_page, float_impl, flow_block,
    flow_footnotes, new_flow_state, para_between, run_paginated_loop,
};

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
pub(super) fn begin_continuous_section(state: &mut FlowState, section: &Section) {
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
