// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `content.xml` table serialisation: `<table:table>` and its rows/cells.

use loki_doc_model::content::table::row::Cell;
use loki_doc_model::content::table::{Row, Table};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::{TableLook, resolve_cell_shading};
use loki_primitives::color::DocumentColor;

use super::content::{Cx, write_block};
use super::xml::attr;

/// Writes a `<table:table>` (header rows, then bodies, then footer). Each
/// cell's effective background — its direct shading, else the table style's
/// banding resolved for that grid position — is baked into a per-cell
/// automatic style (ODF has no conditional-region concept).
pub(super) fn table(out: &mut String, t: &Table, cx: &mut Cx) {
    out.push_str("<table:table>");
    let cols = t.col_specs.len().max(1);
    out.push_str(&format!(
        "<table:table-column table:number-columns-repeated=\"{cols}\"/>"
    ));

    let rows = flatten_rows(t);
    let col_count = grid_col_count(&rows, t.col_specs.len());
    let cell_cols = assign_grid_columns(&rows, col_count);
    // Phase 1: resolve every cell's effective background (immutable borrow of
    // the style catalog), so phase 2 can borrow `cx.auto` mutably.
    let backgrounds = resolve_backgrounds(t, cx, &rows, &cell_cols, col_count);

    // Phase 2: emit rows/cells, minting the per-cell automatic styles.
    for (r, row) in rows.iter().enumerate() {
        out.push_str("<table:table-row>");
        for (ci, cell) in row.cells.iter().enumerate() {
            table_cell(out, cell, backgrounds[r][ci].as_ref(), cx);
        }
        out.push_str("</table:table-row>");
    }
    out.push_str("</table:table>");
}

/// The rows of `t` in visual order: header rows, then each body's rows, then
/// footer rows.
fn flatten_rows(t: &Table) -> Vec<&Row> {
    let mut rows: Vec<&Row> = t.head.rows.iter().collect();
    for body in &t.bodies {
        rows.extend(body.head_rows.iter().chain(body.body_rows.iter()));
    }
    rows.extend(t.foot.rows.iter());
    rows
}

/// The grid column count: the declared columns, or the widest row's summed
/// column spans, whichever is larger.
fn grid_col_count(rows: &[&Row], declared: usize) -> usize {
    let widest = rows
        .iter()
        .map(|r| r.cells.iter().map(|c| c.col_span as usize).sum())
        .max()
        .unwrap_or(0);
    declared.max(widest).max(1)
}

/// Each cell's starting grid column, accounting for `col_span` and for columns
/// covered by a `row_span` cell from an earlier row (vertical merges). Mirrors
/// the layout engine's `assign_cell_columns`.
fn assign_grid_columns(rows: &[&Row], col_count: usize) -> Vec<Vec<usize>> {
    let mut covered = vec![vec![false; col_count]; rows.len()];
    let mut result = Vec::with_capacity(rows.len());
    for (r, row) in rows.iter().enumerate() {
        let mut col = 0usize;
        let mut starts = Vec::with_capacity(row.cells.len());
        for cell in &row.cells {
            while col < col_count && covered[r][col] {
                col += 1;
            }
            let start = col.min(col_count);
            let end = (start + cell.col_span as usize).min(col_count);
            starts.push(start);
            if cell.row_span > 1 {
                let last = (r + cell.row_span as usize).min(rows.len());
                for cov in covered.iter_mut().take(last).skip(r + 1) {
                    cov[start..end].fill(true);
                }
            }
            col = end;
        }
        result.push(starts);
    }
    result
}

/// The effective background for every cell: its direct shading, else the
/// referenced table style's banding resolved for the cell's grid position.
fn resolve_backgrounds(
    t: &Table,
    cx: &Cx,
    rows: &[&Row],
    cell_cols: &[Vec<usize>],
    col_count: usize,
) -> Vec<Vec<Option<DocumentColor>>> {
    let style = t
        .style_name()
        .and_then(|n| cx.table_styles.get(&StyleId::new(n)));
    let look = t
        .table_look_code()
        .and_then(TableLook::decode_attr)
        .unwrap_or_default();
    let n_rows = rows.len();
    rows.iter()
        .enumerate()
        .map(|(r, row)| {
            row.cells
                .iter()
                .enumerate()
                .map(|(ci, cell)| {
                    cell.props.background_color.clone().or_else(|| {
                        style.and_then(|s| {
                            resolve_cell_shading(s, &look, r, cell_cols[r][ci], n_rows, col_count)
                        })
                    })
                })
                .collect()
        })
        .collect()
}

fn table_cell(out: &mut String, cell: &Cell, background: Option<&DocumentColor>, cx: &mut Cx) {
    out.push_str("<table:table-cell");
    if let Some(style) = cx.auto.cell_style(background) {
        attr(out, "table:style-name", &style);
    }
    if cell.col_span > 1 {
        attr(
            out,
            "table:number-columns-spanned",
            &cell.col_span.to_string(),
        );
    }
    if cell.row_span > 1 {
        attr(out, "table:number-rows-spanned", &cell.row_span.to_string());
    }
    out.push('>');
    if cell.blocks.is_empty() {
        out.push_str("<text:p/>");
    } else {
        for b in &cell.blocks {
            write_block(out, b, cx);
        }
    }
    out.push_str("</table:table-cell>");
    for _ in 1..cell.col_span {
        out.push_str("<table:covered-table-cell/>");
    }
}
