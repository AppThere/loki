// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Layout-side table-style shading resolution.
//!
//! Bridges the pure `loki_doc_model::style::resolve_cell_shading` banding
//! resolver into the flow engine: look up a table's named style in the
//! catalog, then compute the shading it contributes to each grid cell.

use loki_doc_model::StyleCatalog;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::style::table_padding::CellPadding;
use loki_doc_model::style::{
    CellEdges, StyleId, TableBorders, TableLook, TableStyle, resolve_cell_shading,
};
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
///
/// `borders` must come from
/// [`StyleCatalog::table_borders_for`](loki_doc_model::style::StyleCatalog::table_borders_for),
/// which walks the `basedOn` chain — reading `style.table_props.borders` off a
/// single style misses every style that inherits its grid from a parent.
pub fn cell_style_borders(
    borders: Option<&TableBorders>,
    row: usize,
    col: usize,
    rows: usize,
    cols: usize,
) -> CellEdges {
    loki_doc_model::style::table_borders::resolve_cell_borders(borders, row, col, rows, cols)
}

#[cfg(test)]
#[path = "table_shading_tests.rs"]
mod tests;

/// Everything a table's named style contributes to its cells, resolved **once**
/// per table and **through the `basedOn` chain**.
///
/// The flow engine used to carry a bare `Option<&TableStyle>` and read
/// `table_props.*` off it at each use site. That is a flat lookup: DOCX's
/// *Table Grid* is `w:basedOn` *Normal Table*, and Word parks its default
/// `w:tblCellMar` on *Normal Table*, so the properties that matter most are
/// usually reachable only by walking the chain. Resolving into this struct at
/// the table's entry point means no pass can accidentally do the flat read —
/// the unresolved style is simply not what gets passed around.
pub struct TableStyleCtx<'a> {
    /// The named style itself — still needed for banding/conditional shading,
    /// which resolves per region rather than per property.
    pub style: Option<&'a TableStyle>,
    /// The six-sided border set, resolved through the chain.
    pub borders: Option<TableBorders>,
    /// The default cell padding, resolved through the chain.
    pub padding: CellPadding,
}

impl<'a> TableStyleCtx<'a> {
    /// Resolves a table's style context from the catalog.
    #[must_use]
    pub fn resolve(catalog: &'a StyleCatalog, style_name: Option<&str>) -> Self {
        Self {
            style: resolve_table_style(catalog, style_name),
            borders: catalog.table_borders_for(style_name),
            padding: catalog.table_cell_padding_for(style_name),
        }
    }

    /// The `(top, right, bottom, left)` borders this style contributes to the
    /// cell at `(row, col)`.
    #[must_use]
    pub fn cell_edges(&self, row: usize, col: usize, rows: usize, cols: usize) -> CellEdges {
        cell_style_borders(self.borders.as_ref(), row, col, rows, cols)
    }

    /// A cell's effective `(top, bottom, left, right)` padding: the cell's own
    /// value per side, else this style's default for that side.
    #[must_use]
    pub fn cell_padding(
        &self,
        props: &loki_doc_model::content::table::row::CellProps,
    ) -> (
        Option<loki_primitives::units::Points>,
        Option<loki_primitives::units::Points>,
        Option<loki_primitives::units::Points>,
        Option<loki_primitives::units::Points>,
    ) {
        loki_doc_model::style::effective_cell_padding(
            (
                props.padding_top,
                props.padding_bottom,
                props.padding_left,
                props.padding_right,
            ),
            &self.padding,
        )
    }
}
