// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF table → [`Block::Table`] mapping.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};

use crate::odt::mapper::props::map_cell_props;
use crate::odt::model::tables::OdfTable;

use super::context::OdfMappingContext;
use super::paragraphs::map_paragraph;

pub(crate) fn map_table(table: &OdfTable, ctx: &mut OdfMappingContext<'_>) -> Block {
    // COMPAT(odf): column width from style:table-column-properties
    // Expand repeated column definitions, resolving fixed widths from style lookup.
    let col_specs: Vec<ColSpec> = table
        .col_defs
        .iter()
        .flat_map(|def| {
            let count = def.columns_repeated.max(1) as usize;
            let width = def
                .style_name
                .as_deref()
                .and_then(|name| ctx.col_style_widths.get(name))
                .map_or(ColWidth::Proportional(1.0), |&pts| ColWidth::Fixed(pts));
            let spec = ColSpec {
                alignment: ColAlignment::Default,
                width,
            };
            std::iter::repeat_n(spec, count)
        })
        .collect();

    let body_rows: Vec<Row> = table
        .rows
        .iter()
        .map(|odf_row| {
            let cells: Vec<Cell> = odf_row
                .cells
                .iter()
                .filter_map(|odf_cell| {
                    // Covered cells are suppressed; the spanning cell carries
                    // `row_span` from `table:number-rows-spanned` (read by the reader).
                    if odf_cell.is_covered {
                        return None;
                    }
                    let blocks: Vec<Block> = odf_cell
                        .paragraphs
                        .iter()
                        .flat_map(|p| {
                            let block = map_paragraph(p, ctx);
                            let figs = std::mem::take(&mut ctx.pending_figures);
                            std::iter::once(block).chain(figs)
                        })
                        .collect();
                    // NOTE: ODF cell properties are mapped to the same CellProps
                    // type as OOXML. The layout engine applies them identically.
                    let props = odf_cell
                        .style_name
                        .as_deref()
                        .and_then(|n| ctx.cell_style_props.get(n))
                        .map(map_cell_props)
                        .unwrap_or_default();
                    Some(Cell {
                        attr: NodeAttr::default(),
                        alignment: ColAlignment::Default,
                        row_span: odf_cell.row_span,
                        col_span: odf_cell.col_span,
                        blocks,
                        props,
                    })
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    Block::Table(Box::new(Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs,
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(body_rows)],
        foot: TableFoot::empty(),
    }))
}
