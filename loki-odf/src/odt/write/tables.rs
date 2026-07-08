// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `content.xml` table serialisation: `<table:table>` and its rows/cells.

use loki_doc_model::content::table::{Row, Table};

use super::content::{Cx, write_block};
use super::xml::attr;

/// Writes a `<table:table>` (header rows, then bodies, then footer).
pub(super) fn table(out: &mut String, t: &Table, cx: &mut Cx) {
    out.push_str("<table:table>");
    let cols = t.col_specs.len().max(1);
    out.push_str(&format!(
        "<table:table-column table:number-columns-repeated=\"{cols}\"/>"
    ));
    for row in &t.head.rows {
        table_row(out, row, cx);
    }
    for body in &t.bodies {
        for row in body.head_rows.iter().chain(body.body_rows.iter()) {
            table_row(out, row, cx);
        }
    }
    for row in &t.foot.rows {
        table_row(out, row, cx);
    }
    out.push_str("</table:table>");
}

fn table_row(out: &mut String, row: &Row, cx: &mut Cx) {
    out.push_str("<table:table-row>");
    for cell in &row.cells {
        out.push_str("<table:table-cell");
        // Direct cell shading → an automatic `table-cell` style (ODF's
        // per-cell representation of table shading / banding).
        if let Some(style) = cx.auto.cell_style(&cell.props) {
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
    out.push_str("</table:table-row>");
}
