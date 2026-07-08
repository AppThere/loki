// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table geometry helpers for the flow engine: cell-height measurement,
//! column-width resolution, nested cell-block flowing, and grid-column
//! assignment (vertical-merge coverage). Split out of `flow.rs` (Phase 7.1);
//! the main `flow_table` orchestrator stays in `flow.rs` and calls these.

use std::collections::HashMap;

use loki_doc_model::StyleCatalog;
use loki_doc_model::content::block::Block;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutSize};
use crate::items::PositionedItem;
use crate::mode::LayoutMode;
use crate::resolve::pts_to_f32;

use super::{FlowState, flow_block, get_items_max_x};

pub(super) fn measure_cell_height(
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    cell: &loki_doc_model::content::table::row::Cell,
    cell_content_width: f32,
    idx: usize,
) -> f32 {
    use loki_doc_model::content::table::row::CellTextDirection;

    let pad_top = cell.props.padding_top.map(pts_to_f32).unwrap_or(0.0);
    let pad_bottom = cell.props.padding_bottom.map(pts_to_f32).unwrap_or(0.0);

    let is_rotated = matches!(
        cell.props.text_direction.as_ref(),
        Some(CellTextDirection::TbRl | CellTextDirection::TbLr | CellTextDirection::BtLr)
    );

    let flow_w = if is_rotated {
        10000.0
    } else {
        cell_content_width
    };

    let mut temp_state = FlowState {
        resources,
        catalog,
        mode: &LayoutMode::Pageless,
        display_scale,
        options,
        cursor_y: 0.0,
        content_width: flow_w,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::default(),
        margins: LayoutInsets::default(),
        page_content_height: 0.0,
        page_number: 1,
        warnings: Vec::new(),
        current_indent: 0.0,
        list_counters: HashMap::new(),
        prev_list_id: None,
        note_counter: 0,
        pending_footnotes: Vec::new(),
        current_paragraphs: Vec::new(),
        checkpoints: Vec::new(),
        // Table cells are always laid out single-column.
        columns: 1,
        column_gap: 0.0,
        column_separator: false,
        col_index: 0,
        column_top_y: 0.0,
        column_item_start: 0,
        column_para_start: 0,
        // Cells never render the comment gutter panel.
        comments: &[],
        pending_comment_anchors: Vec::new(),
        // Cell content: break over-long words to the column width (Word).
        break_long_words: true,
        active_float: None,
        nested_editing: None,
    };

    for block in &cell.blocks {
        flow_block(&mut temp_state, block, idx);
    }

    if is_rotated {
        let max_x = get_items_max_x(&temp_state.current_items);
        max_x + pad_top + pad_bottom
    } else {
        let content_h = temp_state.cursor_y;
        content_h + pad_top + pad_bottom
    }
}

pub(super) fn resolve_column_widths(
    state: &FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
) -> Vec<f32> {
    use loki_doc_model::content::table::col::{ColWidth, TableWidth};

    let col_count = tbl.col_count().max(1);
    let table_width = match tbl.width.as_ref() {
        Some(TableWidth::Fixed(w)) => *w,
        Some(TableWidth::Percent(p)) => state.content_width * (p / 100.0),
        _ => state.content_width,
    };
    let table_width = table_width.max(0.0);

    let mut resolved_widths = vec![0.0f32; col_count];
    let mut proportional_shares = vec![0.0f32; col_count];
    let mut total_fixed_width = 0.0f32;
    let mut total_proportional_shares = 0.0f32;

    for i in 0..col_count {
        let spec = tbl.col_specs.get(i);
        let width_spec = spec.map(|s| s.width).unwrap_or(ColWidth::Default);
        match width_spec {
            ColWidth::Fixed(pts) => {
                let w = pts_to_f32(pts);
                resolved_widths[i] = w;
                total_fixed_width += w;
            }
            ColWidth::Proportional(share) => {
                proportional_shares[i] = share;
                total_proportional_shares += share;
            }
            ColWidth::Default | _ => {
                proportional_shares[i] = 1.0;
                total_proportional_shares += 1.0;
            }
        }
    }

    let remaining_width = (table_width - total_fixed_width).max(0.0);
    if total_proportional_shares > 0.0 {
        let share_unit = remaining_width / total_proportional_shares;
        for i in 0..col_count {
            let spec = tbl.col_specs.get(i);
            let width_spec = spec.map(|s| s.width).unwrap_or(ColWidth::Default);
            match width_spec {
                ColWidth::Proportional(_) | ColWidth::Default => {
                    resolved_widths[i] = proportional_shares[i] * share_unit;
                }
                _ => {}
            }
        }
    } else if total_fixed_width > 0.0 {
        // Fixed-layout tables (`w:tblLayout="fixed"`) honour the grid widths
        // exactly — the table overflows or underfills rather than rescaling.
        // Autofit tables scale the fixed widths to fill the table width.
        let fixed_layout = tbl
            .attr
            .classes
            .iter()
            .any(|c| c == loki_doc_model::content::table::core::TABLE_FIXED_LAYOUT_CLASS);
        if !fixed_layout {
            let scale = table_width / total_fixed_width;
            for w in &mut resolved_widths {
                *w *= scale;
            }
        }
    } else {
        let uniform_w = table_width / col_count as f32;
        resolved_widths.fill(uniform_w);
    }

    resolved_widths
}

// Helper to layout cell blocks inside a nested flow state.
// Helper requires passing all context values to configure the FlowState.
#[allow(clippy::too_many_arguments)]
pub(super) fn flow_cell_blocks(
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    blocks: &[Block],
    content_width: f32,
    starting_indent: f32,
    starting_y: f32,
    idx: usize,
) -> Vec<PositionedItem> {
    let mut temp_state = FlowState {
        resources,
        catalog,
        mode: &LayoutMode::Pageless,
        display_scale,
        options,
        cursor_y: starting_y,
        content_width,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::default(),
        margins: LayoutInsets::default(),
        page_content_height: 0.0,
        page_number: 1,
        warnings: Vec::new(),
        current_indent: starting_indent,
        list_counters: HashMap::new(),
        prev_list_id: None,
        note_counter: 0,
        pending_footnotes: Vec::new(),
        current_paragraphs: Vec::new(),
        checkpoints: Vec::new(),
        // Table cells are always laid out single-column.
        columns: 1,
        column_gap: 0.0,
        column_separator: false,
        col_index: 0,
        column_top_y: 0.0,
        column_item_start: 0,
        column_para_start: 0,
        // Cells never render the comment gutter panel.
        comments: &[],
        pending_comment_anchors: Vec::new(),
        // Cell content: break over-long words to the column width (Word).
        break_long_words: true,
        active_float: None,
        nested_editing: None,
    };

    for block in blocks {
        flow_block(&mut temp_state, block, idx);
    }

    temp_state.current_items
}

/// Assign each cell its grid column span `(col_start, col_end)`, accounting for
/// columns occupied by a `row_span` (vMerge) cell from an earlier row.
///
/// Walks rows top-to-bottom, left-to-right: each cell takes the next column not
/// already covered by a vertical merge from above, then occupies `col_span`
/// columns. A cell with `row_span > 1` marks its columns covered in the rows it
/// spans, so cells there skip those columns. Mirrors the OOXML/HTML table grid.
pub(super) fn assign_cell_columns(
    rows: &[&loki_doc_model::content::table::row::Row],
    col_count: usize,
) -> Vec<Vec<(usize, usize)>> {
    let mut covered = vec![vec![false; col_count]; rows.len()];
    let mut cell_cols: Vec<Vec<(usize, usize)>> = Vec::with_capacity(rows.len());
    for (row_idx, row) in rows.iter().enumerate() {
        let mut col = 0usize;
        let mut row_cols = Vec::with_capacity(row.cells.len());
        for cell in &row.cells {
            while col < col_count && covered[row_idx][col] {
                col += 1;
            }
            let col_start = col.min(col_count);
            let col_end = (col_start + cell.col_span as usize).min(col_count);
            row_cols.push((col_start, col_end));
            if cell.row_span > 1 {
                let last = (row_idx + cell.row_span as usize).min(rows.len());
                for cov_row in covered.iter_mut().take(last).skip(row_idx + 1) {
                    cov_row[col_start..col_end].fill(true);
                }
            }
            col = col_end;
        }
        cell_cols.push(row_cols);
    }
    cell_cols
}
