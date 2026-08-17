// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-row cell background/border decoration emission (flow pass 3b). Split out
//! of `flow.rs` (Phase 7.1); row-height measurement (passes 1–2) moved on to
//! `flow_table_measure.rs` in the file-ceiling pass. `flow_table` (in
//! `flow_table_main.rs`) orchestrates and calls both.

use loki_doc_model::content::table::row::Row;
use loki_doc_model::style::TableLook;

use crate::geometry::{LayoutPoint, LayoutRect, LayoutSize};
use crate::items::{PositionedBorderRect, PositionedItem, PositionedRect};
use crate::resolve::{convert_border, resolve_color};
use crate::table_shading::{TableStyleCtx, cell_style_shading_cnf};

use super::FlowState;

#[path = "flow_table_measure.rs"]
pub(super) mod measure;

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
    style_ctx: &TableStyleCtx<'_>,
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

            // A direct cell border wins; otherwise the table style's borders
            // (e.g. the Table Grid style's outer edges + interior gridlines)
            // fill in each edge, so a styled table draws its grid without the
            // cells carrying explicit borders. `sb` is (top, right, bottom, left).
            let sb = style_ctx.cell_edges(row_idx, col_start, grid_rows, grid_cols);
            let eff = loki_doc_model::style::table_borders::effective_cell_edges(
                (
                    cell.props.border_top.as_ref(),
                    cell.props.border_right.as_ref(),
                    cell.props.border_bottom.as_ref(),
                    cell.props.border_left.as_ref(),
                ),
                &sb,
            );
            let (eff_top, eff_right, eff_bottom, eff_left) = (
                eff.0.as_ref(),
                eff.1.as_ref(),
                eff.2.as_ref(),
                eff.3.as_ref(),
            );

            let has_borders = eff_top.is_some()
                || eff_bottom.is_some()
                || eff_left.is_some()
                || eff_right.is_some();

            // Direct cell shading wins, else the table style's banding — via
            // the cell's explicit w:cnfStyle mask when it carries one (4a.3).
            let cell_bg = cell.props.background_color.clone().or_else(|| {
                cell_style_shading_cnf(
                    style_ctx.style,
                    look,
                    cell.cnf_code(),
                    row_idx,
                    col_start,
                    grid_rows,
                    grid_cols,
                )
            });

            let is_first = p == cell_page_start;
            let is_last = p == row_page_end;

            let border_top = if is_first {
                eff_top.and_then(convert_border)
            } else {
                None
            };
            let border_bottom = if is_last {
                eff_bottom.and_then(convert_border)
            } else {
                None
            };
            let border_left = eff_left.and_then(convert_border);
            let border_right = eff_right.and_then(convert_border);

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
                // A `w:shd` line/cross texture paints as a hatch (bg + lines);
                // a plain fill paints as a flat rect. Direct cell shading only —
                // table-style banding has no texture.
                if let Some(shading) = cell.props.shading.as_ref() {
                    items.insert(
                        insert_idx,
                        PositionedItem::HatchRect(crate::resolve::hatch_from_shading(
                            shading, cell_rect,
                        )),
                    );
                } else if let Some(bg) = cell_bg.as_ref() {
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
