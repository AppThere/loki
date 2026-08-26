// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `content.xml` table serialisation: `<table:table>` and its rows/cells.

use loki_doc_model::content::table::row::Cell;
use loki_doc_model::content::table::{Row, Table};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::table_banding::resolve_cell_shading_cnf;
use loki_doc_model::style::table_borders::{CellEdges, effective_cell_edges, resolve_cell_borders};
use loki_doc_model::style::{TableCnf, TableLook, resolve_cell_shading};
use loki_primitives::color::DocumentColor;

use super::content::Cx;
use super::xml::attr;

/// Writes a `<table:table>` (header rows, then bodies, then footer). Each
/// cell's effective background — its direct shading, else the table style's
/// banding resolved for that grid position — is baked into a per-cell
/// automatic style (ODF has no conditional-region concept).
pub(super) fn table(out: &mut String, t: &Table, cx: &mut Cx) {
    out.push_str("<table:table");
    // The table-level named style (width / alignment / background), defined in
    // styles.xml. Banding is baked per cell below, not carried here.
    if let Some(style) = t.style_name() {
        attr(out, "table:style-name", style);
    }
    out.push('>');
    let cols = t.col_specs.len().max(1);
    out.push_str(&format!(
        "<table:table-column table:number-columns-repeated=\"{cols}\"/>"
    ));

    let rows = flatten_rows(t);
    let col_count = grid_col_count(&rows, t.col_specs.len());
    let cell_cols = assign_grid_columns(&rows, col_count);
    // Phase 1: resolve every cell's effective background *and borders*
    // (immutable borrow of the style catalog), so phase 2 can borrow `cx.auto`
    // mutably. ODF represents both per cell, so both must be resolved here —
    // a table-level border set has no ODF-native form to defer to.
    let backgrounds = resolve_backgrounds(t, cx, &rows, &cell_cols, col_count);
    let borders = resolve_borders(t, cx, &rows, &cell_cols, col_count);
    let paddings = resolve_paddings(t, cx, &rows);

    // Phase 2: emit rows/cells, minting the per-cell automatic styles.
    for (r, row) in rows.iter().enumerate() {
        out.push_str("<table:table-row>");
        for (ci, cell) in row.cells.iter().enumerate() {
            table_cell(
                out,
                cell,
                backgrounds[r][ci].as_ref(),
                &borders[r][ci],
                &paddings[r][ci],
                cx,
            );
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
        .and_then(|n| cx.styles.table_styles.get(&StyleId::new(n)));
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
                            // An explicit w:cnfStyle mask (4a.3) beats the
                            // positional derivation, matching the paint path.
                            match cell.cnf_code().and_then(TableCnf::decode_attr) {
                                Some(cnf) => resolve_cell_shading_cnf(s, &cnf),
                                None => resolve_cell_shading(
                                    s,
                                    &look,
                                    r,
                                    cell_cols[r][ci],
                                    n_rows,
                                    col_count,
                                ),
                            }
                        })
                    })
                })
                .collect()
        })
        .collect()
}

/// The effective `(top, right, bottom, left)` borders for every cell: its
/// direct edges, else the referenced table style's edge for that grid
/// position. Resolved per edge by the same helper the paint path uses, so an
/// exported table draws the grid it drew on screen.
fn resolve_borders(
    t: &Table,
    cx: &Cx,
    rows: &[&Row],
    cell_cols: &[Vec<usize>],
    col_count: usize,
) -> Vec<Vec<CellEdges>> {
    // Chain-resolved, and with the table's own `w:tblBorders` layered over the
    // style's: a style deriving its grid from a parent (Table Grid is `basedOn`
    // Normal Table) contributes nothing under a flat lookup, and a table's
    // direct set is invisible to a style-only one.
    let borders = cx.styles.table_borders_in_force(t);
    let n_rows = rows.len();
    rows.iter()
        .enumerate()
        .map(|(r, row)| {
            row.cells
                .iter()
                .enumerate()
                .map(|(ci, cell)| {
                    let from_style = resolve_cell_borders(
                        borders.as_ref(),
                        r,
                        cell_cols[r][ci],
                        n_rows,
                        col_count,
                    );
                    effective_cell_edges(
                        (
                            cell.props.border_top.as_ref(),
                            cell.props.border_right.as_ref(),
                            cell.props.border_bottom.as_ref(),
                            cell.props.border_left.as_ref(),
                        ),
                        &from_style,
                    )
                })
                .collect()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
/// A cell's effective `(top, bottom, left, right)` padding.
pub(super) type EffectivePadding = (
    Option<loki_primitives::units::Points>,
    Option<loki_primitives::units::Points>,
    Option<loki_primitives::units::Points>,
    Option<loki_primitives::units::Points>,
);

/// The effective padding for every cell: its direct `padding_*`, else the
/// referenced table style's `w:tblCellMar` default for that side.
///
/// ODF has no table-level cell-margin concept — like shading and borders, the
/// inset is represented per cell — so a style's contribution must be baked in
/// here or it is simply lost on save. Unlike borders this does not vary by
/// grid position: `w:tblCellMar` is one value for the whole table.
fn resolve_paddings(t: &Table, cx: &Cx, rows: &[&Row]) -> Vec<Vec<EffectivePadding>> {
    let from_style = cx.styles.table_cell_padding_for(t.style_name());
    rows.iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| {
                    loki_doc_model::style::effective_cell_padding(
                        (
                            cell.props.padding_top,
                            cell.props.padding_bottom,
                            cell.props.padding_left,
                            cell.props.padding_right,
                        ),
                        &from_style,
                    )
                })
                .collect()
        })
        .collect()
}

fn table_cell(
    out: &mut String,
    cell: &Cell,
    background: Option<&DocumentColor>,
    edges: &CellEdges,
    padding: &EffectivePadding,
    cx: &mut Cx,
) {
    out.push_str("<table:table-cell");
    if let Some(style) = cx.auto.cell_style(background, edges, padding) {
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
        super::list_write::write_blocks(out, &cell.blocks, cx);
    }
    out.push_str("</table:table-cell>");
    for _ in 1..cell.col_span {
        out.push_str("<table:covered-table-cell/>");
    }
}
