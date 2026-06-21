// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table mapper: `w:tbl` → `Block::Table`.

use std::collections::{HashMap, HashSet};

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth, TableWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{
    Cell, CellProps, CellTextDirection, CellVerticalAlign, Row,
};
use loki_primitives::units::Points;

use crate::docx::model::styles::{
    DocxTableModel, DocxTableRow, DocxTextDirection, DocxVAlign, DocxVMerge,
};

use super::document::MappingContext;
use super::paragraph::map_paragraph;
use super::props::map_border_edge;

/// Maps a `w:tbl` to a `Block::Table`.
///
/// ## Vertical merge
/// A two-pass algorithm resolves `w:vMerge` spans: restart cells receive
/// the correct `row_span` count and continuation cells are removed from the
/// output (OOXML §17.4.84).
///
/// ## Column widths
/// `w:tblGrid` widths are converted from twips to points when present;
/// otherwise `ColWidth::Default` is used.
pub(crate) fn map_table(t: &DocxTableModel, ctx: &mut MappingContext<'_>) -> Block {
    let col_specs = build_col_specs(t);

    // Pre-compute row spans and the set of continuation cells to remove.
    let (span_map, skip_set) = compute_v_merge_spans(&t.rows);

    let mut head_rows: Vec<Row> = Vec::new();
    let mut body_rows: Vec<Row> = Vec::new();

    for (row_idx, tr) in t.rows.iter().enumerate() {
        let is_header = tr.tr_pr.as_ref().is_some_and(|p| p.is_header);

        let mut grid_col: usize = 0;
        let mut cells: Vec<Cell> = Vec::new();
        for (cell_idx, tc) in tr.cells.iter().enumerate() {
            let col_span = tc
                .tc_pr
                .as_ref()
                .and_then(|p| p.grid_span)
                .unwrap_or(1)
                .max(1) as usize;
            // NOTE: continuation cells are removed from output. The spanning
            // cell above carries row_span = N covering all removed rows.
            // loki-layout must account for row_span when placing cell content.
            if !skip_set.contains(&(row_idx, cell_idx)) {
                let mut cell = map_cell(tc, ctx);
                cell.row_span = span_map.get(&(row_idx, grid_col)).copied().unwrap_or(1);
                cells.push(cell);
            }
            grid_col += col_span;
        }

        let row = Row::new(cells);
        if is_header {
            head_rows.push(row);
        } else {
            body_rows.push(row);
        }
    }

    let head = if head_rows.is_empty() {
        TableHead::empty()
    } else {
        TableHead {
            attr: NodeAttr::default(),
            rows: head_rows,
        }
    };

    let body = TableBody::from_rows(body_rows);

    let width = map_tbl_width(t);

    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width,
        col_specs,
        head,
        bodies: vec![body],
        foot: TableFoot::empty(),
    };

    Block::Table(Box::new(table))
}

/// Converts `w:tblW` to [`TableWidth`].
///
/// COMPAT(microsoft): w:tblW @w:type="pct" uses fiftieths of a percent,
/// not hundredths — divide by 50 to get 0.0–100.0 range.
#[allow(clippy::cast_precision_loss)] // twip values are small; f32 precision is sufficient
fn map_tbl_width(t: &DocxTableModel) -> Option<TableWidth> {
    let w = t.tbl_pr.as_ref()?.width.as_ref()?;
    Some(match w.w_type.as_str() {
        "dxa" => TableWidth::Fixed(w.w as f32 / 20.0),
        "pct" => TableWidth::Percent(w.w as f32 / 50.0),
        _ => TableWidth::Auto, // "auto" | "nil" | unknown
    })
}

/// Builds column specifications from `w:tblGrid` column widths.
fn build_col_specs(t: &DocxTableModel) -> Vec<ColSpec> {
    if t.col_widths.is_empty() {
        // Fall back: infer column count from the widest row.
        let num_cols = t.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
        (0..num_cols)
            .map(|_| ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            })
            .collect()
    } else {
        t.col_widths
            .iter()
            .map(|&w| ColSpec {
                alignment: ColAlignment::Default,
                width: if w > 0 {
                    ColWidth::Fixed(Points::new(f64::from(w) / 20.0))
                } else {
                    ColWidth::Default
                },
            })
            .collect()
    }
}

/// Maps a `w:tc` table cell.
fn map_cell(tc: &crate::docx::model::styles::DocxTableCell, ctx: &mut MappingContext<'_>) -> Cell {
    let col_span = tc.tc_pr.as_ref().and_then(|p| p.grid_span).unwrap_or(1);
    let blocks: Vec<Block> = tc
        .paragraphs
        .iter()
        .flat_map(|p| map_paragraph(p, ctx))
        .collect();

    let mut props = CellProps::default();
    if let Some(tc_pr) = tc.tc_pr.as_ref() {
        // Cell background from `w:shd @w:fill`.
        if let Some(ref hex) = tc_pr.shd_fill
            && let Some(rgb) = crate::xml_util::hex_color(hex)
        {
            use loki_primitives::color::DocumentColor;
            props.background_color = Some(DocumentColor::Rgb(rgb));
        }
        // Cell borders from `w:tcBorders`.
        if let Some(ref borders) = tc_pr.tc_borders {
            props.border_top = borders.top.as_ref().map(map_border_edge);
            props.border_bottom = borders.bottom.as_ref().map(map_border_edge);
            props.border_left = borders.left.as_ref().map(map_border_edge);
            props.border_right = borders.right.as_ref().map(map_border_edge);
        }
        // Cell padding from `w:tcMar`. COMPAT(ooxml-dxa): twips ÷ 20 = points.
        if let Some(ref m) = tc_pr.tc_margins {
            props.padding_top = m.top.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_bottom = m.bottom.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_left = m.left.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_right = m.right.map(|v| Points::new(f64::from(v) / 20.0));
        }
        // Vertical alignment from `w:vAlign`.
        props.vertical_align = tc_pr.v_align.map(|v| match v {
            DocxVAlign::Top => CellVerticalAlign::Top,
            DocxVAlign::Center => CellVerticalAlign::Middle,
            DocxVAlign::Bottom => CellVerticalAlign::Bottom,
        });
        // Text direction from `w:textDirection`.
        props.text_direction = tc_pr.text_direction.map(|d| match d {
            DocxTextDirection::LrTb => CellTextDirection::LrTb,
            DocxTextDirection::TbRl => CellTextDirection::TbRl,
            DocxTextDirection::TbLr => CellTextDirection::TbLr,
            DocxTextDirection::BtLr => CellTextDirection::BtLr,
        });
    }

    Cell {
        attr: NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span: 1, // overridden by map_table after compute_v_merge_spans
        col_span,
        blocks,
        props,
    }
}

// ── vMerge two-pass algorithm ─────────────────────────────────────────────────

/// Computes `row_span` values for all vertically-merged cells in a table.
///
/// Returns:
/// - `span_map`: `(row_idx, grid_col)` → `row_span` for every `Restart` cell.
///   The key uses the cell's *starting* grid column (accounting for
///   `w:gridSpan` of preceding cells in the same row).
/// - `skip_set`: `(row_idx, cell_idx)` pairs that are `Continue` cells and
///   should be omitted from the output row.
///
/// OOXML §17.4.84: `w:vMerge` with no `w:val` is a continuation cell.
///
/// # Algorithm
///
/// **Pass 1** — build a `v_merge_grid[row][grid_col]` by expanding each cell
/// by its `w:gridSpan` so that multi-column cells fill multiple grid slots
/// with the same vMerge state.
///
/// **Pass 2** — for each grid column, scan down; on every `Restart` cell,
/// count consecutive `Continue` cells below and record the span length.
/// Each counted `Continue` cell is added to `skip_set`.
#[allow(clippy::type_complexity)] // Pre-existing pattern — structural refactor deferred
fn compute_v_merge_spans(
    rows: &[DocxTableRow],
) -> (HashMap<(usize, usize), u32>, HashSet<(usize, usize)>) {
    // Pass 1: expand cells into a flat grid indexed by grid column.
    let mut v_merge_grid: Vec<Vec<Option<DocxVMerge>>> = Vec::with_capacity(rows.len());
    let mut cell_idx_grid: Vec<Vec<usize>> = Vec::with_capacity(rows.len());

    for row in rows {
        let mut v_merge_row: Vec<Option<DocxVMerge>> = Vec::new();
        let mut cell_idx_row: Vec<usize> = Vec::new();
        for (cell_idx, cell) in row.cells.iter().enumerate() {
            let v_merge = cell.tc_pr.as_ref().and_then(|p| p.v_merge);
            let col_span = cell
                .tc_pr
                .as_ref()
                .and_then(|p| p.grid_span)
                .unwrap_or(1)
                .max(1) as usize;
            for _ in 0..col_span {
                v_merge_row.push(v_merge);
                cell_idx_row.push(cell_idx);
            }
        }
        v_merge_grid.push(v_merge_row);
        cell_idx_grid.push(cell_idx_row);
    }

    let num_rows = v_merge_grid.len();
    let num_cols = v_merge_grid.iter().map(Vec::len).max().unwrap_or(0);

    let mut span_map: HashMap<(usize, usize), u32> = HashMap::new();
    // COMPAT(microsoft): w:vMerge with no w:val attribute is a continuation
    // cell per OOXML §17.4.84, not a restart. Some non-Microsoft producers
    // incorrectly omit w:vMerge entirely for continuation cells — those will
    // still render as row_span=1.
    let mut skip_set: HashSet<(usize, usize)> = HashSet::new();

    // Pass 2: for each column, find restart cells and count their span.
    for col in 0..num_cols {
        for row in 0..num_rows {
            if v_merge_grid[row].get(col).copied() == Some(Some(DocxVMerge::Restart)) {
                let mut span = 1u32;
                let mut r = row + 1;
                while r < num_rows
                    && v_merge_grid[r].get(col).copied() == Some(Some(DocxVMerge::Continue))
                {
                    if let Some(&cell_idx) = cell_idx_grid[r].get(col) {
                        skip_set.insert((r, cell_idx));
                    }
                    span += 1;
                    r += 1;
                }
                span_map.insert((row, col), span);
            }
        }
    }

    (span_map, skip_set)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
