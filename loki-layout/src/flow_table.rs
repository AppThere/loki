// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Main table layout entry point (`flow_table`).
//!
//! Cell measurement and column-width resolution live in
//! [`crate::flow_table_measure`]. Row-decoration placement (borders, fills)
//! lives in [`crate::flow_table_decor`].

use loki_doc_model::content::table::row::{CellTextDirection, CellVerticalAlign};

use crate::flow::FlowState;
use crate::flow_block::finish_page;
use crate::flow_block::flow_block;
use crate::flow_table_decor::place_row_decorations;
use crate::flow_table_measure::{
    flow_cell_blocks, get_items_max_x, measure_cell_height, resolve_column_widths,
};
use crate::geometry::LayoutPoint;
use crate::items::PositionedItem;
use crate::resolve::pts_to_f32;

/// Flow a table block into `state`, advancing `state.cursor_y` past the table.
pub(crate) fn flow_table(
    state: &mut FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
    idx: usize,
) {
    let col_widths = resolve_column_widths(state, tbl);

    let mut rows = Vec::new();
    rows.extend(&tbl.head.rows);
    for body in &tbl.bodies {
        rows.extend(&body.head_rows);
        rows.extend(&body.body_rows);
    }
    rows.extend(&tbl.foot.rows);

    let mut row_heights = vec![0.0f32; rows.len()];

    // Pass 1: Measure all cells with row_span == 1
    for (row_idx, row) in rows.iter().enumerate() {
        let mut col_start = 0;
        for cell in &row.cells {
            let col_end = (col_start + cell.col_span as usize).min(col_widths.len());
            if cell.row_span == 1 {
                let pad_left = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0);
                let pad_right = cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);
                let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
                let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);
                let h = measure_cell_height(
                    state.resources,
                    state.catalog,
                    state.display_scale,
                    state.options,
                    cell,
                    cell_content_width,
                    idx,
                );
                row_heights[row_idx] = row_heights[row_idx].max(h);
            }
            col_start = col_end;
        }
        row_heights[row_idx] = row_heights[row_idx].max(crate::MIN_ROW_HEIGHT);
    }

    // Pass 2: Distribute spanning cell heights across spanned rows
    for (row_idx, row) in rows.iter().enumerate() {
        let mut col_start = 0;
        for cell in &row.cells {
            let col_end = (col_start + cell.col_span as usize).min(col_widths.len());
            if cell.row_span > 1 {
                let span = cell.row_span as usize;
                let spanned_height: f32 = row_heights
                    [row_idx..(row_idx + span).min(row_heights.len())]
                    .iter()
                    .sum();
                let pad_left = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0);
                let pad_right = cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);
                let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
                let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);
                let needed = measure_cell_height(
                    state.resources,
                    state.catalog,
                    state.display_scale,
                    state.options,
                    cell,
                    cell_content_width,
                    idx,
                );
                if needed > spanned_height {
                    let extra = needed - spanned_height;
                    let last = (row_idx + span - 1).min(row_heights.len() - 1);
                    row_heights[last] += extra;
                }
            }
            col_start = col_end;
        }
    }

    // Pass 3: Place and flow cell blocks
    for (row_idx, row) in rows.iter().enumerate() {
        let row_max_h = row_heights[row_idx];

        if state.mode.is_paginated() {
            let remaining_h = state.page_content_height - state.cursor_y;
            if row_max_h > remaining_h && row_max_h <= state.page_content_height {
                finish_page(state);
            }
        }

        let original_row_page = state.page_number;
        let original_row_y_start = state.cursor_y;
        let mut row_y_start = original_row_y_start;
        let mut row_page = original_row_page;

        let table_indent = state.current_indent;
        let mut cell_starts = Vec::new();

        // Pass 3a: Flow cell content blocks
        let mut col_start = 0;
        for (c_idx, cell) in row.cells.iter().enumerate() {
            let col_end = (col_start + cell.col_span as usize).min(col_widths.len());
            let old_indent = state.current_indent;
            let old_width = state.content_width;

            let pad_top = cell.props.padding_top.map(pts_to_f32).unwrap_or(0.0);
            let pad_bottom = cell.props.padding_bottom.map(pts_to_f32).unwrap_or(0.0);
            let pad_left = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0);
            let pad_right = cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);

            let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
            let cell_x = old_indent + col_widths[0..col_start].iter().sum::<f32>();
            let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);

            let cell_height = if cell.row_span == 1 {
                row_max_h
            } else {
                let span = cell.row_span as usize;
                row_heights[row_idx..(row_idx + span).min(row_heights.len())]
                    .iter()
                    .sum()
            };

            if state.page_number != row_page {
                row_y_start = state.cursor_y;
                row_page = state.page_number;
            }

            if state.page_number == original_row_page {
                state.cursor_y = original_row_y_start + pad_top;
            } else {
                state.cursor_y = 0.0 + pad_top;
            }

            cell_starts.push((state.page_number, state.current_items.len()));

            let rotation_degrees = match cell.props.text_direction.as_ref() {
                Some(CellTextDirection::TbRl) => Some(90.0_f32),
                Some(CellTextDirection::TbLr) => Some(270.0_f32),
                Some(CellTextDirection::BtLr) => Some(270.0_f32),
                _ => None,
            };

            let cell_items = if let Some(degrees) = rotation_degrees {
                // NOTE(cell-rotation): for rotated cells, content is laid out
                // with width/height swapped, then the RotatedGroup transform
                // visually rotates the result into the correct orientation.
                let rotated_content_width = (cell_height - pad_top - pad_bottom).max(0.0);
                let inner_items = flow_cell_blocks(
                    state.resources,
                    state.catalog,
                    state.display_scale,
                    state.options,
                    &cell.blocks,
                    rotated_content_width,
                    pad_top,
                    pad_left,
                    idx,
                );

                let max_x = get_items_max_x(&inner_items);
                let content_visual_height = max_x;
                let cell_avail_h = (cell_height - pad_top - pad_bottom).max(0.0);
                let extra_space = (cell_avail_h - content_visual_height).max(0.0);
                let y_offset = match cell.props.vertical_align {
                    Some(CellVerticalAlign::Middle) => extra_space / 2.0,
                    Some(CellVerticalAlign::Bottom) => extra_space,
                    _ => 0.0,
                };

                vec![PositionedItem::RotatedGroup {
                    origin: LayoutPoint {
                        x: cell_x,
                        y: row_y_start + y_offset,
                    },
                    degrees,
                    content_width: cell_height,
                    content_height: cell_content_width,
                    items: inner_items,
                }]
            } else {
                state.current_indent = cell_x + pad_left;
                state.content_width = cell_content_width;

                for block in &cell.blocks {
                    flow_block(state, block, idx);
                }

                let cell_page_start = cell_starts[c_idx].0;
                let cell_item_start = cell_starts[c_idx].1;
                if cell_page_start == state.page_number {
                    let content_h = (state.cursor_y - (row_y_start + pad_top)).max(0.0);
                    let cell_avail_h = (cell_height - pad_top - pad_bottom).max(0.0);
                    let extra_space = (cell_avail_h - content_h).max(0.0);
                    let y_offset = match cell.props.vertical_align {
                        Some(CellVerticalAlign::Middle) => extra_space / 2.0,
                        Some(CellVerticalAlign::Bottom) => extra_space,
                        _ => 0.0,
                    };
                    if y_offset > 0.0 {
                        for item in &mut state.current_items[cell_item_start..] {
                            item.translate(0.0, y_offset);
                        }
                    }
                }

                Vec::new()
            };

            for item in cell_items {
                state.current_items.push(item);
            }

            state.current_indent = old_indent;
            state.content_width = old_width;
            col_start = col_end;
        }

        let row_page_end = state.page_number;
        let row_y_end = if original_row_page == row_page_end {
            original_row_y_start + row_max_h
        } else {
            let first_h = (state.page_content_height - original_row_y_start).max(0.0);
            let intermediate_h =
                (row_page_end - original_row_page - 1) as f32 * state.page_content_height;
            (row_max_h - first_h - intermediate_h).max(0.0)
        };

        // Pass 3b: Emit background and border decorations for this row's cells
        place_row_decorations(
            state,
            row,
            &col_widths,
            &row_heights,
            &cell_starts,
            row_idx,
            table_indent,
            original_row_page,
            original_row_y_start,
            row_page_end,
        );

        state.cursor_y = row_y_end;
    }
}
