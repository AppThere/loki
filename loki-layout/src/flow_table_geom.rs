// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table geometry helpers for the flow engine: cell-height measurement,
//! column-width resolution, nested cell-block flowing, and grid-column
//! assignment (vertical-merge coverage). Split out of `flow.rs` (Phase 7.1);
//! the main `flow_table` orchestrator stays in `flow.rs` and calls these.

use loki_doc_model::StyleCatalog;
use loki_doc_model::content::block::Block;

use crate::LayoutOptions;
use crate::font::FontResources;
use crate::items::PositionedItem;
use crate::resolve::pts_to_f32;
use crate::result::PageParagraphData;

use super::{FlowState, editing, flow_block, get_items_max_x};

#[allow(clippy::too_many_arguments)] // the flow_cell_blocks precedent
pub(super) fn measure_cell_height(
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    cell: &loki_doc_model::content::table::row::Cell,
    cell_content_width: f32,
    idx: usize,
    cell_chars: Option<&loki_doc_model::style::props::char_props::CharProps>,
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

    let mut temp_state = super::table_autofit::cell_flow_state(
        resources,
        catalog,
        display_scale,
        options,
        flow_w,
        0.0,
        // Cell content: break over-long words to the column width (Word).
        true,
        cell_chars,
    );

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
    state: &mut FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
    rows: &[&loki_doc_model::content::table::row::Row],
    cell_cols: &[Vec<(usize, usize)>],
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
            let scaled: Vec<f32> = resolved_widths.iter().map(|w| w * scale).collect();
            // Word autofit: honour the scaled preferred widths, but guarantee
            // each column at least its minimum content width so a too-narrow
            // preferred width can't force one-character-per-line wrapping.
            resolved_widths = super::table_autofit::autofit_column_widths(
                state,
                rows,
                cell_cols,
                &scaled,
                table_width,
            );
        }
    } else {
        let uniform_w = table_width / col_count as f32;
        resolved_widths.fill(uniform_w);
    }

    resolved_widths
}

/// Flow a rotated cell's blocks in an isolated (content-local) coordinate
/// frame, returning both the positioned items and the per-paragraph editing
/// data. Each paragraph is tagged with the cell's `NestedEditing` path (using
/// `cell_flat`, the bridge's flat `KEY_TABLE_CELLS` index) so a click can
/// resolve to the live cell; origins stay in the content-local frame, to be
/// mapped to the page by the caller's [`crate::result::CellRotation`].
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
    cell_flat: usize,
    cell_chars: Option<&loki_doc_model::style::props::char_props::CharProps>,
) -> (Vec<PositionedItem>, Vec<PageParagraphData>) {
    let mut temp_state = super::table_autofit::cell_flow_state(
        resources,
        catalog,
        display_scale,
        options,
        content_width,
        starting_indent,
        // Cell content: break over-long words to the column width (Word).
        true,
        cell_chars,
    );
    temp_state.cursor_y = starting_y;

    for (bi, block) in blocks.iter().enumerate() {
        // Tag cell paragraphs so a click in the rotated cell resolves to it.
        temp_state.nested_editing = Some(editing::NestedEditing::cell(idx, cell_flat, bi));
        flow_block(&mut temp_state, block, idx);
    }
    temp_state.nested_editing = None;

    (temp_state.current_items, temp_state.current_paragraphs)
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
