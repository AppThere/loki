// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Layout-side table-style shading resolution.
//!
//! Bridges the pure `loki_doc_model::style::resolve_cell_shading` banding
//! resolver into the flow engine: look up a table's named style in the
//! catalog, then compute the shading it contributes to each grid cell.

use loki_doc_model::StyleCatalog;
use loki_doc_model::style::{StyleId, TableLook, TableStyle, resolve_cell_shading};
use loki_primitives::color::DocumentColor;

/// The named table style a table references, if any, resolved against the
/// document's style catalog.
pub fn resolve_table_style<'a>(
    catalog: &'a StyleCatalog,
    style_name: Option<&str>,
) -> Option<&'a TableStyle> {
    style_name.and_then(|name| catalog.table_styles.get(&StyleId::new(name)))
}

/// The background a table style contributes to the cell at `(row, col)` in a
/// `rows`×`cols` grid, honoring OOXML region/banding precedence.
///
/// The active `w:tblLook` flags are not yet imported, so Word's default
/// (`04A0`: header row + first column + row banding) is assumed.
///
/// TODO(table-tbllook-import): thread the table's real `w:tblLook`.
pub fn cell_style_shading(
    style: Option<&TableStyle>,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> Option<DocumentColor> {
    style.and_then(|s| resolve_cell_shading(s, &TableLook::default(), row, col, rows, cols))
}

#[cfg(test)]
#[path = "table_shading_tests.rs"]
mod tests;
