// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table serialisation to XHTML `<table>` for the EPUB content document.

use loki_doc_model::content::table::core::Table;
use loki_doc_model::content::table::row::{Cell, Row};

use crate::content::RenderCtx;

impl RenderCtx {
    /// Renders a [`Table`] as an XHTML `<table>` with optional `<thead>`,
    /// `<tbody>`, and `<tfoot>` sections. Cell `colspan`/`rowspan` are honoured.
    pub(crate) fn render_table(&mut self, table: &Table, out: &mut String) {
        out.push_str("<table>\n");

        if !table.head.rows.is_empty() {
            out.push_str("<thead>\n");
            for row in &table.head.rows {
                self.render_row(row, "th", out);
            }
            out.push_str("</thead>\n");
        }

        out.push_str("<tbody>\n");
        for body in &table.bodies {
            // Per-body head rows repeat as header cells.
            for row in &body.head_rows {
                self.render_row(row, "th", out);
            }
            for row in &body.body_rows {
                self.render_row(row, "td", out);
            }
        }
        out.push_str("</tbody>\n");

        if !table.foot.rows.is_empty() {
            out.push_str("<tfoot>\n");
            for row in &table.foot.rows {
                self.render_row(row, "td", out);
            }
            out.push_str("</tfoot>\n");
        }

        out.push_str("</table>\n");
    }

    fn render_row(&mut self, row: &Row, cell_tag: &str, out: &mut String) {
        out.push_str("<tr>");
        for cell in &row.cells {
            self.render_cell(cell, cell_tag, out);
        }
        out.push_str("</tr>\n");
    }

    fn render_cell(&mut self, cell: &Cell, tag: &str, out: &mut String) {
        let mut attrs = String::new();
        if cell.col_span > 1 {
            attrs.push_str(&format!(" colspan=\"{}\"", cell.col_span));
        }
        if cell.row_span > 1 {
            attrs.push_str(&format!(" rowspan=\"{}\"", cell.row_span));
        }
        out.push_str(&format!("<{tag}{attrs}>"));
        for block in &cell.blocks {
            self.render_block(block, out);
        }
        out.push_str(&format!("</{tag}>"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::render_content;
    use loki_doc_model::Document;
    use loki_doc_model::content::block::Block;
    use loki_doc_model::content::inline::Inline;
    use loki_doc_model::content::table::core::{Table, TableBody, TableFoot, TableHead};

    #[test]
    fn renders_table_with_header_and_body() {
        let header = Row::new(vec![Cell::simple(vec![Block::Para(vec![Inline::Str(
            "H".into(),
        )])])]);
        let body = Row::new(vec![Cell::simple(vec![Block::Para(vec![Inline::Str(
            "C".into(),
        )])])]);
        let table = Table {
            attr: Default::default(),
            caption: Default::default(),
            width: None,
            col_specs: Vec::new(),
            head: TableHead {
                attr: Default::default(),
                rows: vec![header],
            },
            bodies: vec![TableBody::from_rows(vec![body])],
            foot: TableFoot::empty(),
        };

        let mut doc = Document::new();
        let sec = doc.first_section_mut().unwrap();
        sec.blocks.clear();
        sec.blocks.push(Block::Table(Box::new(table)));

        let rendered = render_content(&doc);
        assert!(rendered.body.contains("<table>"));
        assert!(rendered.body.contains("<thead>"));
        assert!(rendered.body.contains("<th><p>H</p>"));
        assert!(rendered.body.contains("<td><p>C</p>"));
        assert!(rendered.body.contains("</tbody>"));
    }
}
