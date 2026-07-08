// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Pure resolver for table-style conditional formatting (banding).
//!
//! Given a [`TableStyle`], the active [`TableLook`] flags, and a cell's
//! position, [`resolve_cell_shading`] returns the background color the
//! style contributes to that cell — the merge of the twelve conditional
//! regions plus the base whole-table shading.
//!
//! TR 29166 §7.2.4. OOXML precedence (highest first): the four corner
//! cells, then first/last row, then first/last column, then the horizontal
//! bands, then the vertical bands, then whole-table. Each *property* is
//! resolved independently — a higher-precedence region that does not define
//! shading falls through to the next region that does, rather than blocking
//! it.

use crate::style::table_style::{TableLook, TableProps, TableRegion, TableStyle};
use loki_primitives::color::DocumentColor;

/// Region precedence, highest first. `WholeTable` is last and always
/// applies, so it acts as the conditional fallback before the base shading.
const PRECEDENCE: [TableRegion; 13] = [
    TableRegion::NwCell,
    TableRegion::NeCell,
    TableRegion::SwCell,
    TableRegion::SeCell,
    TableRegion::FirstRow,
    TableRegion::LastRow,
    TableRegion::FirstColumn,
    TableRegion::LastColumn,
    TableRegion::Band1Horz,
    TableRegion::Band2Horz,
    TableRegion::Band1Vert,
    TableRegion::Band2Vert,
    TableRegion::WholeTable,
];

/// Resolve the background color a table style contributes to the cell at
/// `(row, col)` in a `rows`×`cols` grid, honoring the active `look` flags.
///
/// Returns the highest-precedence conditional region that defines shading;
/// if none do, falls back to the style's base table shading
/// ([`TableProps::background_color`]). `None` means the style adds no
/// shading and the cell's own `CellProps` shading (if any) shows through.
pub fn resolve_cell_shading(
    style: &TableStyle,
    look: &TableLook,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> Option<DocumentColor> {
    if rows == 0 || cols == 0 || row >= rows || col >= cols {
        return None;
    }
    let h = horiz_band(look, &style.table_props, row, rows);
    let v = vert_band(look, &style.table_props, col, cols);
    for region in PRECEDENCE {
        if !region_applies(region, look, row, col, rows, cols, h, v) {
            continue;
        }
        // Region matched; if it defines shading it wins, else fall through.
        if let Some(color) = style
            .conditional
            .get(&region)
            .and_then(|f| f.background_color.as_ref())
        {
            return Some(color.clone());
        }
    }
    style.table_props.background_color.clone()
}

/// The horizontal band a row belongs to, or `None` if row banding is off
/// or the row is a header/footer row (excluded from banding).
fn horiz_band(
    look: &TableLook,
    props: &TableProps,
    row: usize,
    rows: usize,
) -> Option<TableRegion> {
    if !look.horizontal_banding {
        return None;
    }
    let lead = usize::from(look.first_row);
    let trail = usize::from(look.last_row);
    if row < lead || row + trail >= rows {
        return None;
    }
    let size = props.row_band_size.unwrap_or(1).max(1) as usize;
    Some(band_parity(
        (row - lead) / size,
        TableRegion::Band1Horz,
        TableRegion::Band2Horz,
    ))
}

/// The vertical band a column belongs to, or `None` if column banding is
/// off or the column is a first/last column (excluded from banding).
fn vert_band(look: &TableLook, props: &TableProps, col: usize, cols: usize) -> Option<TableRegion> {
    if !look.vertical_banding {
        return None;
    }
    let lead = usize::from(look.first_column);
    let trail = usize::from(look.last_column);
    if col < lead || col + trail >= cols {
        return None;
    }
    let size = props.col_band_size.unwrap_or(1).max(1) as usize;
    Some(band_parity(
        (col - lead) / size,
        TableRegion::Band1Vert,
        TableRegion::Band2Vert,
    ))
}

/// Map a zero-based band index to its 1-based parity region: even → band 1
/// (odd stripes), odd → band 2 (even stripes).
fn band_parity(index: usize, band1: TableRegion, band2: TableRegion) -> TableRegion {
    if index.is_multiple_of(2) { band1 } else { band2 }
}

/// Whether `region` covers the cell at `(row, col)`, given the precomputed
/// band memberships `h`/`v`.
#[allow(clippy::too_many_arguments)]
fn region_applies(
    region: TableRegion,
    look: &TableLook,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
    h: Option<TableRegion>,
    v: Option<TableRegion>,
) -> bool {
    let first_row = look.first_row && row == 0;
    let last_row = look.last_row && row + 1 == rows;
    let first_col = look.first_column && col == 0;
    let last_col = look.last_column && col + 1 == cols;
    match region {
        TableRegion::NwCell => first_row && first_col,
        TableRegion::NeCell => first_row && last_col,
        TableRegion::SwCell => last_row && first_col,
        TableRegion::SeCell => last_row && last_col,
        TableRegion::FirstRow => first_row,
        TableRegion::LastRow => last_row,
        TableRegion::FirstColumn => first_col,
        TableRegion::LastColumn => last_col,
        TableRegion::Band1Horz => h == Some(TableRegion::Band1Horz),
        TableRegion::Band2Horz => h == Some(TableRegion::Band2Horz),
        TableRegion::Band1Vert => v == Some(TableRegion::Band1Vert),
        TableRegion::Band2Vert => v == Some(TableRegion::Band2Vert),
        TableRegion::WholeTable => true,
    }
}

#[cfg(test)]
#[path = "table_banding_tests.rs"]
mod tests;
