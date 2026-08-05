// SPDX-License-Identifier: Apache-2.0

//! `Table` → RSX for the DOM reflow view, so T7.3's scrollport has the element
//! it exists for.
//!
//! # Why the table comes first
//!
//! T7.3's hard case is the element that **cannot** be scaled: a table's width is
//! its column widths, and fitting it to the column means redistributing them,
//! which is a different problem from scaling a rectangle. On the canvas path
//! that table was simply clipped at the tile. Here it overflows into
//! [`super::oversized::AtOversized`], and the reader can reach the rest.
//!
//! # Column widths are stated, not measured
//!
//! `table-layout: fixed` with an explicit `width` per column: a fixed column is
//! its points, a proportional one its share, and a content-sized one is left to
//! the layout. Letting the browser measure instead would make the DOM path
//! decide column widths by a rule `loki-layout` does not use, and the two views
//! would disagree about a table for a reason that is not rendering.
//!
//! # Not covered
//!
//! Cell borders and shading beyond a hairline, banding, the `cnf` conditional
//! regions, and vertical merges (`row_span` is emitted, but a spanned-over cell
//! is not removed because the model does not mark one). `TODO(dom-reflow-table)`.

use dioxus::prelude::*;
use loki_doc_model::content::table::col::{ColAlignment, ColWidth};
use loki_doc_model::content::table::core::Table;
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::style::catalog::StyleCatalog;

use super::content::{FamilyMap, block_el};

/// A hairline, so a cell's extent is visible without claiming to render the
/// document's own borders — which this does not read yet.
const CELL_BORDER: &str = "1px solid #cccccc";

/// One table, at its stated column widths.
pub(super) fn table_el(table: &Table, catalog: &StyleCatalog, families: &FamilyMap) -> Element {
    let rows: Vec<(&Row, bool)> = table
        .head
        .rows
        .iter()
        .map(|r| (r, true))
        .chain(
            table
                .bodies
                .iter()
                .flat_map(|b| b.head_rows.iter().map(|r| (r, true))),
        )
        .chain(
            table
                .bodies
                .iter()
                .flat_map(|b| b.body_rows.iter().map(|r| (r, false))),
        )
        .chain(table.foot.rows.iter().map(|r| (r, false)))
        .collect();

    rsx! {
        table {
            style: "table-layout: fixed; border-collapse: collapse;",
            colgroup {
                for (i, spec) in table.col_specs.iter().enumerate() {
                    { rsx! { col { key: "{i}", style: col_css(&spec.width) } } }
                }
            }
            for (ri, (row, is_head)) in rows.iter().enumerate() {
                {
                    rsx! {
                        tr {
                            key: "{ri}",
                            for (ci, cell) in row.cells.iter().enumerate() {
                                {
                                    rsx! {
                                        td {
                                            key: "{ci}",
                                            colspan: "{cell.col_span}",
                                            rowspan: "{cell.row_span}",
                                            style: cell_css(cell, table, ci, *is_head),
                                            { cell_content(cell, catalog, families) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The `<col>` width for one column spec.
///
/// A content-sized column gets no width at all rather than a guessed one: `auto`
/// under `table-layout: fixed` divides the remaining space, which is what
/// "decided by content" means once the other columns have taken theirs.
fn col_css(width: &ColWidth) -> String {
    match width {
        ColWidth::Fixed(pt) => format!("width: {}pt;", pt.value()),
        // A proportion of the table, expressed the way CSS expresses one. The
        // shares are normalised by the browser across the row, which is the same
        // arithmetic `loki-layout` does over the grid.
        ColWidth::Proportional(share) => format!("width: {}%;", share * 100.0),
        // `Default` and anything a later model version adds: no width at all
        // rather than a guessed one — see the doc comment.
        _ => String::new(),
    }
}

/// One cell's own declarations: alignment, the hairline, and padding.
fn cell_css(cell: &Cell, table: &Table, index: usize, is_head: bool) -> String {
    // The cell's override first, then the column's default — the model's own
    // order, and the order `loki-layout` resolves in.
    let align = match cell.alignment {
        ColAlignment::Default => table
            .col_specs
            .get(index)
            .map_or(ColAlignment::Default, |c| c.alignment),
        other => other,
    };
    let align = match align {
        ColAlignment::Left => "left",
        ColAlignment::Center => "center",
        ColAlignment::Right => "right",
        _ => "start",
    };
    let weight = if is_head { "600" } else { "400" };
    format!(
        "border: {CELL_BORDER}; padding: 2pt 4pt; vertical-align: top; \
         text-align: {align}; font-weight: {weight};"
    )
}

/// A cell's blocks, through the same renderer the document body uses.
///
/// Recursion rather than a cell-specific path: a paragraph in a cell resolves
/// its style through the same catalog as one outside it, and a second renderer
/// would be a second answer to what a style means.
fn cell_content(cell: &Cell, catalog: &StyleCatalog, families: &FamilyMap) -> Element {
    rsx! {
        for (i, block) in cell.blocks.iter().enumerate() {
            { rsx! { div { key: "{i}", { block_el(block, catalog, families) } } } }
        }
    }
}

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
