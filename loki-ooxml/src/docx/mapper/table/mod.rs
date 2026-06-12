// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table mapper: `w:tbl` → `Block::Table`.

pub(crate) mod cell;
pub(crate) mod col_specs;
pub(crate) mod vmerge;

#[cfg(test)]
mod tests;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::Row;

use crate::docx::model::styles::DocxTableModel;

use super::document::MappingContext;

use cell::map_cell;
use col_specs::{build_col_specs, map_tbl_width};
use vmerge::compute_v_merge_spans;

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
        let mut cells: Vec<_> = Vec::new();
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
