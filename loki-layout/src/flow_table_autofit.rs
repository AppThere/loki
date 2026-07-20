// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Autofit table column-width resolution (Word's min/max-content behaviour) and
//! the shared cell-measurement [`FlowState`] builder used by the table
//! geometry passes.
//!
//! A `w:tblLayout="autofit"` table (the OOXML default when no `tblLayout` is
//! present) does **not** honour the preferred grid widths literally the way a
//! fixed-layout table does. Word first guarantees every column at least its
//! *minimum content width* — the widest unbreakable unit (longest word) in any
//! of its cells — and only then distributes the surplus by the preferred
//! widths. Without that guarantee a column whose preferred width is far
//! narrower than its content (e.g. a 0.28"/400-twip callout label holding the
//! word "INSIGHT") is kept pathologically narrow, so the word wraps one
//! character per line and the row grows absurdly tall. This module restores the
//! minimum-content guarantee so such columns widen to fit, matching Word.

use std::collections::HashMap;

use loki_doc_model::StyleCatalog;
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::style::props::char_props::CharProps;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutSize};
use crate::mode::LayoutMode;
use crate::resolve::pts_to_f32;

use super::{FlowState, flow_block, get_items_max_x};

/// Build the isolated, single-column [`FlowState`] every cell-measurement pass
/// uses: `Pageless`, no comment gutter, no column machinery. `break_long_words`
/// and `content_width` vary by caller — height/content measurement flows at the
/// real cell width and breaks over-long words to it (Word); min-content
/// measurement flows at ~0 width **without** long-word breaking so each word
/// lands on its own line.
#[allow(clippy::too_many_arguments)]
pub(super) fn cell_flow_state<'a>(
    resources: &'a mut FontResources,
    catalog: &'a StyleCatalog,
    display_scale: f32,
    options: &'a LayoutOptions,
    content_width: f32,
    starting_indent: f32,
    break_long_words: bool,
    cell_chars: Option<&CharProps>,
) -> FlowState<'a> {
    FlowState {
        resources,
        catalog,
        mode: &LayoutMode::Pageless,
        display_scale,
        options,
        cursor_y: 0.0,
        content_width,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::default(),
        margins: LayoutInsets::default(),
        page_content_height: 0.0,
        rendering_footnotes: false,
        page_number: 1,
        warnings: Vec::new(),
        current_indent: starting_indent,
        list_counters: HashMap::new(),
        prev_list_id: None,
        note_counter: 0,
        pending_footnotes: Vec::new(),
        footnote_reserved: 0.0,
        current_paragraphs: Vec::new(),
        checkpoints: Vec::new(),
        columns: 1,
        column_widths: Vec::new(),
        column_gap: 0.0,
        column_separator: false,
        col_index: 0,
        column_top_y: 0.0,
        column_item_start: 0,
        column_para_start: 0,
        comments: &[],
        pending_comment_anchors: Vec::new(),
        break_long_words,
        active_float: None,
        nested_editing: None,
        staged_between: None,
        tail_candidate: None,
        cell_char_defaults: cell_chars.cloned(),
        line_num: None,
    }
}

/// The minimum content width of a single cell: the widest line extent when the
/// cell is flowed at ~0 width without breaking long words, i.e. every word on
/// its own line. Cell padding is added by the caller.
fn measure_cell_min_width(
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    cell: &Cell,
) -> f32 {
    let mut state = cell_flow_state(
        resources,
        catalog,
        display_scale,
        options,
        1.0,
        0.0,
        false,
        None,
    );
    for block in &cell.blocks {
        // `idx` only tags editing paths; measured items are discarded.
        flow_block(&mut state, block, 0);
    }
    get_items_max_x(&state.current_items)
}

/// Per-column minimum content width: over every single-column cell, the max of
/// its `measure_cell_min_width` plus horizontal padding. Column-spanning cells
/// don't pin a single column, so they're skipped (their content still fits
/// because the spanned columns sum to the cell width).
fn column_min_widths(
    state: &mut FlowState,
    rows: &[&Row],
    cell_cols: &[Vec<(usize, usize)>],
    col_count: usize,
) -> Vec<f32> {
    // Copy the shared refs/scalars out first so `state.resources` can be
    // reborrowed mutably in the loop without conflicting.
    let catalog = state.catalog;
    let display_scale = state.display_scale;
    let options = state.options;

    let mut mins = vec![0.0f32; col_count];
    for (row_idx, row) in rows.iter().enumerate() {
        for (c_idx, cell) in row.cells.iter().enumerate() {
            let (col_start, col_end) = cell_cols[row_idx][c_idx];
            if col_end - col_start != 1 || col_start >= col_count {
                continue;
            }
            let pad = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0)
                + cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);
            let w = measure_cell_min_width(state.resources, catalog, display_scale, options, cell)
                + pad;
            if w > mins[col_start] {
                mins[col_start] = w;
            }
        }
    }
    mins
}

/// Distribute `table_width` across columns so each is at least its minimum
/// content width, sharing the remainder in proportion to the preferred
/// (`scaled`) widths. When no column violates its minimum this returns `scaled`
/// unchanged, so well-proportioned tables are unaffected. When the minimums
/// alone exceed `table_width` the columns keep their minimums and the table
/// overflows — Word's behaviour.
fn distribute_with_mins(scaled: &[f32], mins: &[f32], table_width: f32) -> Vec<f32> {
    let n = scaled.len();
    let mut out = scaled.to_vec();
    if (0..n).all(|i| out[i] + 0.01 >= mins[i]) {
        return out;
    }
    let mut pinned = vec![false; n];
    // Each iteration pins at least one more column; at most `n` need pinning.
    for _ in 0..=n {
        let pinned_sum: f32 = (0..n).filter(|&i| pinned[i]).map(|i| mins[i]).sum();
        let free_scaled: f32 = (0..n).filter(|&i| !pinned[i]).map(|i| scaled[i]).sum();
        let remaining = (table_width - pinned_sum).max(0.0);
        let unpinned = (0..n).filter(|&i| !pinned[i]).count().max(1) as f32;
        let share = |i: usize| {
            if free_scaled > 0.0 {
                scaled[i] / free_scaled
            } else {
                1.0 / unpinned
            }
        };
        let mut changed = false;
        for i in 0..n {
            if !pinned[i] && remaining * share(i) + 0.01 < mins[i] {
                pinned[i] = true;
                changed = true;
            }
        }
        if !changed {
            for i in 0..n {
                out[i] = if pinned[i] {
                    mins[i]
                } else {
                    remaining * share(i)
                };
            }
            break;
        }
    }
    out
}

/// Resolve autofit column widths: start from the preferred widths scaled to the
/// table width (`scaled`), then guarantee each column its minimum content
/// width.
pub(super) fn autofit_column_widths(
    state: &mut FlowState,
    rows: &[&Row],
    cell_cols: &[Vec<(usize, usize)>],
    scaled: &[f32],
    table_width: f32,
) -> Vec<f32> {
    let mins = column_min_widths(state, rows, cell_cols, scaled.len());
    distribute_with_mins(scaled, &mins, table_width)
}

#[cfg(test)]
#[path = "flow_table_autofit_tests.rs"]
mod tests;
