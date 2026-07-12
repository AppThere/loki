// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table-region character formatting (4a.3): resolves each cell's merged
//! `w:tblStylePr/w:rPr` defaults into a per-table grid consumed by both the
//! row-height measurement and content-flow passes (they must agree — bold
//! header text is wider *and* taller).
//!
//! A cell's membership comes from its explicit `w:cnfStyle` mask when it
//! carries one (authoritative — matches the shading resolver), else the
//! positional derivation under the table's `w:tblLook`.

use loki_doc_model::content::table::row::Row;
use loki_doc_model::style::TableStyle;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::table_banding::{resolve_cell_char_props, resolve_cell_char_props_cnf};
use loki_doc_model::style::table_cnf::TableCnf;
use loki_doc_model::style::table_style::TableLook;

/// The per-cell region character defaults for one table, indexed
/// `[row][cell]` (parallel to `cell_cols`), or `None` when the table has no
/// style or its style defines no region character formatting — the common
/// case, which then costs nothing downstream.
pub(super) fn cell_char_grid(
    style: Option<&TableStyle>,
    look: &TableLook,
    rows: &[&Row],
    cell_cols: &[Vec<(usize, usize)>],
    grid_rows: usize,
    grid_cols: usize,
) -> Option<Vec<Vec<Option<CharProps>>>> {
    let style = style?;
    if style
        .conditional
        .values()
        .all(|f| f.char_props == CharProps::default())
    {
        return None;
    }
    Some(
        rows.iter()
            .enumerate()
            .map(|(r, row)| {
                row.cells
                    .iter()
                    .enumerate()
                    .map(
                        |(ci, cell)| match cell.cnf_code().and_then(TableCnf::decode_attr) {
                            Some(cnf) => resolve_cell_char_props_cnf(style, &cnf),
                            None => resolve_cell_char_props(
                                style,
                                look,
                                r,
                                cell_cols[r][ci].0,
                                grid_rows,
                                grid_cols,
                            ),
                        },
                    )
                    .collect()
            })
            .collect(),
    )
}
