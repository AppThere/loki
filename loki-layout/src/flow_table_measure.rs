// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table row-height measurement (flow passes 1–2). Split out of
//! `flow_table_paint.rs` (file-ceiling pass), which owns it as a submodule;
//! `flow_table` calls this before
//! the cell-flow and decoration passes.

use loki_doc_model::content::table::row::Row;

use crate::resolve::pts_to_f32;
use crate::table_shading::TableStyleCtx;

use crate::flow::{FlowState, table_geom};

/// Measure each row's height. Pass 1 sizes cells with `row_span == 1`; pass 2
/// grows the last spanned row when a `row_span > 1` cell needs more than its
/// rows currently provide. Returns one height per row (min `MIN_ROW_HEIGHT`).
pub(in crate::flow) fn measure_row_heights(
    state: &mut FlowState,
    rows: &[&Row],
    cell_cols: &[Vec<(usize, usize)>],
    col_widths: &[f32],
    idx: usize,
    char_grid: Option<&Vec<Vec<Option<loki_doc_model::style::props::char_props::CharProps>>>>,
    style_ctx: &TableStyleCtx<'_>,
) -> Vec<f32> {
    let mut row_heights = vec![0.0f32; rows.len()];

    // Pass 1: Measure all cells with row_span == 1
    for (row_idx, row) in rows.iter().enumerate() {
        for (c_idx, cell) in row.cells.iter().enumerate() {
            let (col_start, col_end) = cell_cols[row_idx][c_idx];
            if cell.row_span == 1 {
                let (_, _, pl, pr) = style_ctx.cell_padding(&cell.props);
                let pad_left = pl.map(pts_to_f32).unwrap_or(0.0);
                let pad_right = pr.map(pts_to_f32).unwrap_or(0.0);
                let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
                let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);
                let h = table_geom::measure_cell_height(
                    state.resources,
                    state.catalog,
                    state.display_scale,
                    state.options,
                    cell,
                    cell_content_width,
                    idx,
                    cell_chars(char_grid, row_idx, c_idx),
                    style_ctx,
                );
                row_heights[row_idx] = row_heights[row_idx].max(h);
            }
        }
        row_heights[row_idx] = row_heights[row_idx].max(crate::MIN_ROW_HEIGHT);
    }

    // Pass 2: Distribute spanning cell heights across spanned rows
    for (row_idx, row) in rows.iter().enumerate() {
        for (c_idx, cell) in row.cells.iter().enumerate() {
            let (col_start, col_end) = cell_cols[row_idx][c_idx];
            if cell.row_span > 1 {
                let span = cell.row_span as usize;
                let spanned_height: f32 = row_heights
                    [row_idx..(row_idx + span).min(row_heights.len())]
                    .iter()
                    .sum();
                let (_, _, pl, pr) = style_ctx.cell_padding(&cell.props);
                let pad_left = pl.map(pts_to_f32).unwrap_or(0.0);
                let pad_right = pr.map(pts_to_f32).unwrap_or(0.0);
                let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
                let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);
                let needed = table_geom::measure_cell_height(
                    state.resources,
                    state.catalog,
                    state.display_scale,
                    state.options,
                    cell,
                    cell_content_width,
                    idx,
                    cell_chars(char_grid, row_idx, c_idx),
                    style_ctx,
                );
                if needed > spanned_height {
                    let extra = needed - spanned_height;
                    let last = (row_idx + span - 1).min(row_heights.len() - 1);
                    row_heights[last] += extra;
                }
            }
        }
    }

    row_heights
}

/// The 4a.3 region character defaults for one cell of the grid, if any.
pub(in crate::flow) fn cell_chars(
    grid: Option<&Vec<Vec<Option<loki_doc_model::style::props::char_props::CharProps>>>>,
    row: usize,
    cell: usize,
) -> Option<&loki_doc_model::style::props::char_props::CharProps> {
    grid.and_then(|g| g.get(row))
        .and_then(|r| r.get(cell))
        .and_then(Option::as_ref)
}
