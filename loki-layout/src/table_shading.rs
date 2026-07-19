// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Layout-side table-style shading resolution.
//!
//! Bridges the pure `loki_doc_model::style::resolve_cell_shading` banding
//! resolver into the flow engine: look up a table's named style in the
//! catalog, then compute the shading it contributes to each grid cell.

use loki_doc_model::StyleCatalog;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::style::{CellEdges, StyleId, TableLook, TableStyle, resolve_cell_shading};
use loki_primitives::color::DocumentColor;

/// The named table style a table references, if any, resolved against the
/// document's style catalog.
pub fn resolve_table_style<'a>(
    catalog: &'a StyleCatalog,
    style_name: Option<&str>,
) -> Option<&'a TableStyle> {
    style_name.and_then(|name| catalog.table_styles.get(&StyleId::new(name)))
}

/// The table instance's active `w:tblLook` region flags (which of the style's
/// conditional regions apply), or the format default when the table carries
/// no (or a malformed) encoded look.
pub fn table_look(tbl: &Table) -> TableLook {
    tbl.table_look_code()
        .and_then(TableLook::decode_attr)
        .unwrap_or_default()
}

/// The background a table style contributes to the cell at `(row, col)` in a
/// `rows`×`cols` grid, honoring OOXML region/banding precedence under the
/// table instance's active `look`.
pub fn cell_style_shading(
    style: Option<&TableStyle>,
    look: &TableLook,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> Option<DocumentColor> {
    style.and_then(|s| resolve_cell_shading(s, look, row, col, rows, cols))
}

/// [`cell_style_shading`], but honouring an explicit `w:cnfStyle` mask when
/// the cell carries one (4a.3): the mask is authoritative (Word stamped it
/// under the active look), so it replaces the positional derivation; absent
/// or malformed masks fall back to it.
#[allow(clippy::too_many_arguments)] // mirrors cell_style_shading + the mask
pub fn cell_style_shading_cnf(
    style: Option<&TableStyle>,
    look: &TableLook,
    cnf_code: Option<&str>,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> Option<DocumentColor> {
    if let Some(cnf) = cnf_code.and_then(loki_doc_model::style::table_cnf::TableCnf::decode_attr) {
        return style
            .and_then(|s| loki_doc_model::style::table_banding::resolve_cell_shading_cnf(s, &cnf));
    }
    cell_style_shading(style, look, row, col, rows, cols)
}

/// The `(top, right, bottom, left)` borders a table style contributes to the
/// cell at `(row, col)` in a `rows`×`cols` grid — an outer edge on the table
/// boundary, otherwise the interior gridline for that axis. Each edge is `None`
/// where the style leaves it unset, so a caller can fall back to it only when a
/// direct cell border is absent. This is how a *Table Grid* style paints a full
/// grid even though the cells carry no explicit borders.
pub fn cell_style_borders(
    style: Option<&TableStyle>,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> CellEdges {
    style
        .and_then(|s| s.table_props.borders.as_ref())
        .map(|b| b.edges_for(row, col, rows, cols))
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "table_shading_tests.rs"]
mod tests;
