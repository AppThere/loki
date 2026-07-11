// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table pass 3a: flow each cell's block content into the page, honouring
//! per-cell padding, rotation (TbRl/TbLr/BtLr → a `RotatedGroup`), vertical
//! alignment, and single-page clipping. Split out of `flow.rs` (Phase 7.1);
//! `flow_table` calls this once per row and feeds the returned `cell_starts`
//! (page + item index where each cell began) to the pass-3b decoration emitter.

use loki_doc_model::content::table::row::Row;

use crate::geometry::{LayoutPoint, LayoutRect, LayoutSize};
use crate::items::PositionedItem;
use crate::resolve::pts_to_f32;
use crate::result::CellRotation;

use super::{FlowState, editing, flow_block, get_items_max_x, table_geom};

/// Flow one row's cells; returns each cell's `(page, item_start)` for pass 3b.
#[allow(clippy::too_many_arguments)]
pub(super) fn flow_row_cells(
    state: &mut FlowState,
    row: &Row,
    row_idx: usize,
    cell_cols_row: &[(usize, usize)],
    col_widths: &[f32],
    row_heights: &[f32],
    row_max_h: f32,
    original_row_page: usize,
    original_row_y_start: f32,
    idx: usize,
    cell_flat: &mut usize,
) -> Vec<(usize, usize)> {
    use loki_doc_model::content::table::row::{CellTextDirection, CellVerticalAlign};

    let mut row_y_start = original_row_y_start;
    let mut row_page = original_row_page;
    let mut cell_starts = Vec::new();
    for (c_idx, cell) in row.cells.iter().enumerate() {
        let (col_start, col_end) = cell_cols_row[c_idx];
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

        // If a previous cell caused a page break, update row_y_start to the
        // top of the new page so this cell doesn't land in the wrong position.
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
            // NOTE(cell-rotation): content laid out width/height-swapped,
            // then RotatedGroup rotates it (fine for text runs).
            let rotated_content_width = (cell_height - pad_top - pad_bottom).max(0.0);
            let (inner_items, cell_paras) = table_geom::flow_cell_blocks(
                state.resources,
                state.catalog,
                state.display_scale,
                state.options,
                &cell.blocks,
                rotated_content_width,
                pad_top,
                pad_left,
                idx,
                *cell_flat,
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

            // Editing data: the caret maps through the SAME affine the renderer
            // applies to the RotatedGroup (content-local → page), so a click
            // resolves to the right character in the rotated cell. Pivots mirror
            // `loki-vello` scene.rs cx/cy_local + cx/cy_physical (90/270 branch).
            // Hit-testing, the caret (painted tilted via `cursor_paint_transform`
            // in loki-vello), and up/down arrow navigation (`visual_y_span`) are
            // all rotation-aware. See docs/fidelity-status.md §rotated-cells.
            let rotation = CellRotation {
                degrees,
                pivot_local: (cell_height / 2.0, cell_content_width / 2.0),
                pivot_page: (
                    cell_x + cell_content_width / 2.0,
                    (row_y_start + y_offset) + cell_height / 2.0,
                ),
            };
            for mut para in cell_paras {
                para.rotation = Some(rotation);
                state.current_paragraphs.push(para);
            }

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
            // Cell content breaks over-long words to the column width (Word).
            let old_break = state.break_long_words;
            state.break_long_words = true;

            let cell_para_start = state.current_paragraphs.len();
            for (bi, block) in cell.blocks.iter().enumerate() {
                // Tag cell paragraphs so a click resolves to the live cell.
                state.nested_editing = Some(editing::NestedEditing::cell(idx, *cell_flat, bi));
                flow_block(state, block, idx);
            }
            state.nested_editing = None;
            state.break_long_words = old_break;

            // If it fits on a single page, apply vertical alignment
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
                    // Editing origins must follow their translated glyphs so
                    // the caret in a v-aligned cell lands on the text.
                    for para in &mut state.current_paragraphs[cell_para_start..] {
                        para.origin.1 += y_offset;
                    }
                }

                // Clip single-page cell content to its box so over-wide
                // content can't bleed into neighbours (Word). A cell spilling
                // to a later page stays unclipped — see fidelity-status.
                if state.current_items.len() > cell_item_start {
                    let cell_top_y = if state.page_number == original_row_page {
                        original_row_y_start
                    } else {
                        0.0
                    };
                    let clip_rect = LayoutRect {
                        origin: LayoutPoint {
                            x: cell_x,
                            y: cell_top_y,
                        },
                        size: LayoutSize {
                            width: cell_w,
                            height: cell_height,
                        },
                    };
                    let inner: Vec<PositionedItem> =
                        state.current_items.drain(cell_item_start..).collect();
                    state.current_items.push(PositionedItem::ClippedGroup {
                        clip_rect,
                        items: inner,
                    });
                }
            }

            Vec::new()
        };

        for item in cell_items {
            state.current_items.push(item);
        }

        state.current_indent = old_indent;
        state.content_width = old_width;
        *cell_flat += 1;
    }
    cell_starts
}
