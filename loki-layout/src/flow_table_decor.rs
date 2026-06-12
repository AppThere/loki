// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Row-decoration placement: cell background fills and border rects.
//!
//! This module contains [`place_row_decorations`], extracted from
//! `flow_table`'s pass 3b to keep file sizes under the 300-line ceiling.

use loki_doc_model::content::table::row::Row;

use crate::flow::FlowState;
use crate::geometry::{LayoutPoint, LayoutRect, LayoutSize};
use crate::items::{PositionedBorderRect, PositionedItem, PositionedRect};
use crate::resolve::{convert_border, resolve_color};

/// Emit background fills and border [`PositionedItem`]s for one table row.
///
/// `cell_starts` is a parallel slice over `row.cells` where each entry is
/// `(page_number, item_index)` recorded just before the cell's content was
/// flowed. Background fills are inserted *before* content items so they render
/// underneath.
#[allow(clippy::too_many_arguments)]
pub(crate) fn place_row_decorations(
    state: &mut FlowState,
    row: &Row,
    col_widths: &[f32],
    row_heights: &[f32],
    cell_starts: &[(usize, usize)],
    row_idx: usize,
    table_indent: f32,
    original_row_page: usize,
    original_row_y_start: f32,
    row_page_end: usize,
) {
    for p in original_row_page..=row_page_end {
        let mut col_start_map = Vec::new();
        {
            let mut curr_col = 0;
            for cell in &row.cells {
                col_start_map.push(curr_col);
                curr_col = (curr_col + cell.col_span as usize).min(col_widths.len());
            }
        }

        for (c_idx, cell) in row.cells.iter().enumerate().rev() {
            let cell_page_start = cell_starts[c_idx].0;
            let cell_item_start = cell_starts[c_idx].1;

            if p < cell_page_start {
                continue;
            }

            let cell_h = if cell.row_span == 1 {
                // row_heights[row_idx] is row_max_h for span-1 cells
                row_heights[row_idx]
            } else {
                let span = cell.row_span as usize;
                row_heights[row_idx..(row_idx + span).min(row_heights.len())]
                    .iter()
                    .sum()
            };

            let h = cell_height_on_page(
                p,
                cell_page_start,
                cell_h,
                original_row_page,
                original_row_y_start,
                row_page_end,
                state.page_content_height,
            );
            if h < 0.0 || (h == 0.0 && cell_h > 0.0) {
                continue;
            }

            let y = if p == original_row_page {
                original_row_y_start
            } else {
                0.0
            };

            let col_start = col_start_map[c_idx];
            let col_end = (col_start + cell.col_span as usize).min(col_widths.len());
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

            let insert_idx = if p == cell_page_start { cell_item_start } else { 0 };

            if p == state.page_number {
                if has_borders {
                    state.current_items.insert(
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
                if let Some(bg) = cell.props.background_color.as_ref() {
                    state.current_items.insert(
                        insert_idx,
                        PositionedItem::FilledRect(PositionedRect {
                            rect: cell_rect,
                            color: resolve_color(Some(bg)),
                        }),
                    );
                }
            } else if let Some(page) = state.pages.get_mut(p - 1) {
                if has_borders {
                    page.content_items.insert(
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
                if let Some(bg) = cell.props.background_color.as_ref() {
                    page.content_items.insert(
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

/// Compute what portion of `cell_h` falls on page `p`.
fn cell_height_on_page(
    p: usize,
    cell_page_start: usize,
    cell_h: f32,
    original_row_page: usize,
    original_row_y_start: f32,
    row_page_end: usize,
    page_content_height: f32,
) -> f32 {
    if p == cell_page_start {
        if p == row_page_end {
            cell_h
        } else {
            let y_start = if p == original_row_page {
                original_row_y_start
            } else {
                0.0
            };
            (page_content_height - y_start).max(0.0)
        }
    } else if p == row_page_end {
        let start_y = if cell_page_start == original_row_page {
            original_row_y_start
        } else {
            0.0
        };
        let first_h = (page_content_height - start_y).max(0.0);
        let intermediate_h = (row_page_end - cell_page_start - 1) as f32 * page_content_height;
        (cell_h - first_h - intermediate_h).max(0.0)
    } else {
        page_content_height
    }
}
