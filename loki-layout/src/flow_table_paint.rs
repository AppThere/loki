// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table row-height measurement (passes 1–2) and per-row cell background/border
//! decoration emission (pass 3b) for the flow engine. Split out of `flow.rs`
//! (Phase 7.1); `flow_table` (in `flow.rs`) orchestrates and calls these.

use loki_doc_model::content::table::row::Row;
use loki_doc_model::style::{TableLook, TableStyle};

use crate::geometry::{LayoutPoint, LayoutRect, LayoutSize};
use crate::items::{PositionedBorderRect, PositionedItem, PositionedRect};
use crate::resolve::{convert_border, pts_to_f32, resolve_color};
use crate::table_shading::cell_style_shading;

use super::{FlowState, table_geom};

/// Measure each row's height. Pass 1 sizes cells with `row_span == 1`; pass 2
/// grows the last spanned row when a `row_span > 1` cell needs more than its
/// rows currently provide. Returns one height per row (min `MIN_ROW_HEIGHT`).
pub(super) fn measure_row_heights(
    state: &mut FlowState,
    rows: &[&Row],
    cell_cols: &[Vec<(usize, usize)>],
    col_widths: &[f32],
    idx: usize,
) -> Vec<f32> {
    let mut row_heights = vec![0.0f32; rows.len()];

    // Pass 1: Measure all cells with row_span == 1
    for (row_idx, row) in rows.iter().enumerate() {
        for (c_idx, cell) in row.cells.iter().enumerate() {
            let (col_start, col_end) = cell_cols[row_idx][c_idx];
            if cell.row_span == 1 {
                let pad_left = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0);
                let pad_right = cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);
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
                let pad_left = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0);
                let pad_right = cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);
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

/// Emit the background fill and border rects for one row's cells, inserting
/// them beneath the already-placed cell content on each page the row spans.
/// Direct cell shading wins over the table style's banding.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_row_cell_decorations(
    state: &mut FlowState,
    row: &Row,
    row_idx: usize,
    cell_cols_row: &[(usize, usize)],
    col_widths: &[f32],
    row_heights: &[f32],
    row_max_h: f32,
    cell_starts: &[(usize, usize)],
    table_indent: f32,
    table_style: Option<&TableStyle>,
    look: &TableLook,
    grid_rows: usize,
    grid_cols: usize,
    original_row_page: usize,
    original_row_y_start: f32,
    row_page_end: usize,
) {
    // Helper closures to calculate heights and Y coordinates of cell portions per page
    let get_cell_height_on_page = |p: usize, cell_page_start: usize, cell_h: f32| -> f32 {
        if p == cell_page_start {
            if p == row_page_end {
                cell_h
            } else {
                let y_start = if p == original_row_page {
                    original_row_y_start
                } else {
                    0.0
                };
                (state.page_content_height - y_start).max(0.0)
            }
        } else if p == row_page_end {
            let start_y = if cell_page_start == original_row_page {
                original_row_y_start
            } else {
                0.0
            };
            let first_h = (state.page_content_height - start_y).max(0.0);
            let intermediate_h =
                (row_page_end - cell_page_start - 1) as f32 * state.page_content_height;
            (cell_h - first_h - intermediate_h).max(0.0)
        } else {
            state.page_content_height
        }
    };

    let get_cell_y_on_page = |p: usize| -> f32 {
        if p == original_row_page {
            original_row_y_start
        } else {
            0.0
        }
    };

    // Pass 3b: Emit background and border decorations for this row's cells
    for p in original_row_page..=row_page_end {
        for (c_idx, cell) in row.cells.iter().enumerate().rev() {
            let cell_page_start = cell_starts[c_idx].0;
            let cell_item_start = cell_starts[c_idx].1;

            if p < cell_page_start {
                continue;
            }

            let cell_h = if cell.row_span == 1 {
                row_max_h
            } else {
                let span = cell.row_span as usize;
                row_heights[row_idx..(row_idx + span).min(row_heights.len())]
                    .iter()
                    .sum()
            };

            let h = get_cell_height_on_page(p, cell_page_start, cell_h);
            if h < 0.0 || (h == 0.0 && cell_h > 0.0) {
                continue;
            }

            let y = get_cell_y_on_page(p);
            let (col_start, col_end) = cell_cols_row[c_idx];
            let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
            let cell_x = table_indent + col_widths[0..col_start].iter().sum::<f32>();
            let cell_rect = LayoutRect {
                origin: LayoutPoint { x: cell_x, y },
                size: LayoutSize {
                    width: cell_w,
                    height: h,
                },
            };

            let has_borders = cell.props.border_top.is_some()
                || cell.props.border_bottom.is_some()
                || cell.props.border_left.is_some()
                || cell.props.border_right.is_some();

            // Direct cell shading wins, else the table style's banding.
            let cell_bg = cell.props.background_color.clone().or_else(|| {
                cell_style_shading(table_style, look, row_idx, col_start, grid_rows, grid_cols)
            });

            let is_first = p == cell_page_start;
            let is_last = p == row_page_end;

            let border_top = if is_first {
                cell.props.border_top.as_ref().and_then(convert_border)
            } else {
                None
            };
            let border_bottom = if is_last {
                cell.props.border_bottom.as_ref().and_then(convert_border)
            } else {
                None
            };
            let border_left = cell.props.border_left.as_ref().and_then(convert_border);
            let border_right = cell.props.border_right.as_ref().and_then(convert_border);

            let insert_idx = if p == cell_page_start {
                cell_item_start
            } else {
                0
            };

            // Emit into the in-progress page or an already-finished one.
            let target = if p == state.page_number {
                Some(&mut state.current_items)
            } else {
                state.pages.get_mut(p - 1).map(|pg| &mut pg.content_items)
            };
            if let Some(items) = target {
                if has_borders {
                    items.insert(
                        insert_idx,
                        PositionedItem::BorderRect(PositionedBorderRect {
                            rect: cell_rect,
                            top: border_top,
                            bottom: border_bottom,
                            left: border_left,
                            right: border_right,
                        }),
                    );
                }
                if let Some(bg) = cell_bg.as_ref() {
                    items.insert(
                        insert_idx,
                        PositionedItem::FilledRect(PositionedRect {
                            rect: cell_rect,
                            color: resolve_color(Some(bg)),
                        }),
                    );
                }
            }
        }
    }
}
