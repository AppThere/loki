// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The `flow_table` orchestrator: resolves table geometry, style/look and the
//! 4a.3 region-character grid, then drives the three passes (row-height
//! measurement, per-cell content flow, decoration emission). Split out of
//! `flow.rs` (file-ceiling pass).

use super::{FlowState, columns_impl, table_cells, table_chars, table_geom, table_paint};
use crate::table_shading::{TableStyleCtx, table_look};

pub(super) fn flow_table(
    state: &mut FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
    idx: usize,
) {
    let mut rows = Vec::new();
    rows.extend(&tbl.head.rows);
    for body in &tbl.bodies {
        rows.extend(&body.head_rows);
        rows.extend(&body.body_rows);
    }
    rows.extend(&tbl.foot.rows);

    // Assign each cell its grid columns, accounting for columns covered by a
    // `row_span` (vMerge) cell from an earlier row (`cell_cols[row][cell] =
    // (col_start, col_end)`). Without it a cell whose leading column is occupied
    // by a vertical merge above is placed too far left — the TC-DOCX-005 bug.
    // Resolved before column widths because autofit measures per-column content.
    let cell_cols = table_geom::assign_cell_columns(&rows, tbl.col_count().max(1));

    // Resolved before column widths: autofit sizes columns from each cell's
    // content plus its padding, and a style-supplied `w:tblCellMar` is part of
    // that padding. Resolving after would size every column too narrow by the
    // inherited inset.
    let style_ctx = TableStyleCtx::resolve(state.catalog, tbl.style_name());

    let col_widths = table_geom::resolve_column_widths(state, tbl, &rows, &cell_cols, &style_ctx);

    // Named style + `w:tblLook` → conditional/banding shading (under direct).
    let look = table_look(tbl);
    let (grid_rows, grid_cols) = (rows.len(), col_widths.len());
    // Region character formatting (4a.3): per-cell defaults, `None` for
    // styleless / char-free tables so plain tables pay nothing.
    let char_grid = table_chars::cell_char_grid(
        style_ctx.style,
        &look,
        &rows,
        &cell_cols,
        grid_rows,
        grid_cols,
    );

    let row_heights = table_paint::measure::measure_row_heights(
        state,
        &rows,
        &cell_cols,
        &col_widths,
        idx,
        char_grid.as_ref(),
        &style_ctx,
    );

    // Pass 3: Place and flow cell blocks. `cell_flat` counts cells in the bridge's
    // flat `KEY_TABLE_CELLS` order so cell paragraphs get a matching `PathStep::Cell`.
    let mut cell_flat = 0usize;
    for (row_idx, row) in rows.iter().enumerate() {
        let row_max_h = row_heights[row_idx];

        if state.mode.is_paginated() {
            // Remaining space honours this page's footnote reservation so a row
            // does not overlap the footnote band.
            let remaining_h = state.content_bottom() - state.cursor_y;
            if row_max_h > remaining_h && row_max_h <= state.page_content_height {
                // A whole row that fits in a band but not the remaining space
                // moves to the next column (or page).
                //
                // TODO(table-row-split): Word splits the row instead, unless the
                // author set `w:trPr/w:cantSplit` — absent by default, so
                // splitting is the *common* case and moving the row whole is the
                // exception. Neither the split nor `cantSplit` is modelled
                // anywhere in the workspace.
                //
                // **Do not just relax this guard.** A row taller than a whole
                // page already falls through to the split path (the second
                // condition), and that path is broken: measured on
                // `table-row-taller-than-page.docx`, Word renders 3 pages and
                // Loki 2, with the row's borders missing entirely and the cell
                // text clipped mid-line. Relaxing the guard so ordinary rows
                // split as well makes `iris-blueprint-free` go 571 → 1012
                // failing regions, because the overflowing cell's content
                // vanishes rather than continuing. Repair the over-tall path
                // first, then model `cantSplit`, then relax this.
                //
                // Surfaced by `iris-blueprint-free.docx` page 10, where Word
                // opens the page mid-row with the tail of a row carried from
                // page 9 and Loki moves the whole row down: page 9 improves
                // 26 → 4 failing regions and page 10 costs +96, which is the
                // entire difference between that fixture and its original. See
                // docs/fidelity-status.md §10a.
                columns_impl::break_column(state);
            }
        }

        let original_row_page = state.page_number;
        let original_row_y_start = state.cursor_y;
        let table_indent = state.current_indent;

        let cell_starts = table_cells::flow_row_cells(
            state,
            row,
            row_idx,
            &cell_cols[row_idx],
            &col_widths,
            &row_heights,
            row_max_h,
            original_row_page,
            original_row_y_start,
            idx,
            &mut cell_flat,
            char_grid.as_ref().map(|g| g[row_idx].as_slice()),
            &style_ctx,
        );

        let row_page_end = state.page_number;
        let row_y_end = if original_row_page == row_page_end {
            original_row_y_start + row_max_h
        } else {
            let first_h = (state.page_content_height - original_row_y_start).max(0.0);
            let intermediate_h =
                (row_page_end - original_row_page - 1) as f32 * state.page_content_height;
            (row_max_h - first_h - intermediate_h).max(0.0)
        };

        table_paint::emit_row_cell_decorations(
            state,
            row,
            row_idx,
            &cell_cols[row_idx],
            &col_widths,
            &row_heights,
            row_max_h,
            &cell_starts,
            table_indent,
            &style_ctx,
            &look,
            grid_rows,
            grid_cols,
            original_row_page,
            original_row_y_start,
            row_page_end,
        );

        state.cursor_y = row_y_end;
    }
}
