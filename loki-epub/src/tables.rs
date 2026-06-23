// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table serialisation to XHTML `<table>` for the EPUB content document.
//!
//! Carries the structural model (head/body/foot rows, `colspan`/`rowspan`) plus
//! the presentation the reflowable target can honour: the caption, a
//! `<colgroup>` with per-column widths, and resolved horizontal/vertical cell
//! alignment (cell override falling back to the column default).

use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth, TableWidth};
use loki_doc_model::content::table::core::Table;
use loki_doc_model::content::table::row::{Cell, CellVerticalAlign, Row};

use crate::content::RenderCtx;

impl RenderCtx {
    /// Renders a [`Table`] as an XHTML `<table>` with an optional `<caption>`,
    /// `<colgroup>`, and `<thead>`/`<tbody>`/`<tfoot>` sections. Cell
    /// `colspan`/`rowspan`, column widths, and alignment are honoured.
    pub(crate) fn render_table(&mut self, table: &Table, out: &mut String) {
        match table_width_style(table.width) {
            Some(style) => out.push_str(&format!("<table style=\"{style}\">\n")),
            None => out.push_str("<table>\n"),
        }

        if !table.caption.full.is_empty() {
            out.push_str("<caption>");
            self.render_inlines(&table.caption.full, out);
            out.push_str("</caption>\n");
        }

        render_colgroup(&table.col_specs, out);

        if !table.head.rows.is_empty() {
            out.push_str("<thead>\n");
            for row in &table.head.rows {
                self.render_row(row, "th", &table.col_specs, out);
            }
            out.push_str("</thead>\n");
        }

        out.push_str("<tbody>\n");
        for body in &table.bodies {
            // Per-body head rows repeat as header cells.
            for row in &body.head_rows {
                self.render_row(row, "th", &table.col_specs, out);
            }
            for row in &body.body_rows {
                self.render_row(row, "td", &table.col_specs, out);
            }
        }
        out.push_str("</tbody>\n");

        if !table.foot.rows.is_empty() {
            out.push_str("<tfoot>\n");
            for row in &table.foot.rows {
                self.render_row(row, "td", &table.col_specs, out);
            }
            out.push_str("</tfoot>\n");
        }

        out.push_str("</table>\n");
    }

    fn render_row(&mut self, row: &Row, cell_tag: &str, col_specs: &[ColSpec], out: &mut String) {
        out.push_str("<tr>");
        // A simple left-to-right column cursor. It advances by each cell's
        // `col_span`; it does not model rowspan occupancy carried from earlier
        // rows, so column-default alignment may be approximate in tables that mix
        // row spans with per-column alignment — the common cases resolve exactly.
        let mut col = 0usize;
        for cell in &row.cells {
            self.render_cell(cell, cell_tag, col_specs.get(col), out);
            col += cell.col_span.max(1) as usize;
        }
        out.push_str("</tr>\n");
    }

    fn render_cell(
        &mut self,
        cell: &Cell,
        tag: &str,
        col_spec: Option<&ColSpec>,
        out: &mut String,
    ) {
        let mut attrs = String::new();
        if cell.col_span > 1 {
            attrs.push_str(&format!(" colspan=\"{}\"", cell.col_span));
        }
        if cell.row_span > 1 {
            attrs.push_str(&format!(" rowspan=\"{}\"", cell.row_span));
        }
        if let Some(style) = cell_style(cell, col_spec) {
            attrs.push_str(&format!(" style=\"{style}\""));
        }
        out.push_str(&format!("<{tag}{attrs}>"));
        for block in &cell.blocks {
            self.render_block(block, out);
        }
        out.push_str(&format!("</{tag}>"));
    }
}

/// Emits a `<colgroup>` carrying each column's width, when any column declares
/// one. Proportional widths are normalised against the sum of all proportional
/// shares so they become CSS percentages.
fn render_colgroup(col_specs: &[ColSpec], out: &mut String) {
    if col_specs.is_empty() {
        return;
    }
    let proportional_total: f32 = col_specs
        .iter()
        .filter_map(|c| match c.width {
            ColWidth::Proportional(share) => Some(share),
            _ => None,
        })
        .sum();

    let mut any_width = false;
    let mut cols = String::new();
    for spec in col_specs {
        match col_width_style(spec.width, proportional_total) {
            Some(style) => {
                any_width = true;
                cols.push_str(&format!("<col style=\"{style}\"/>\n"));
            }
            None => cols.push_str("<col/>\n"),
        }
    }
    // Only bother emitting the group if at least one column carries a width;
    // a colgroup of bare <col/>s would be inert.
    if any_width {
        out.push_str("<colgroup>\n");
        out.push_str(&cols);
        out.push_str("</colgroup>\n");
    }
}

/// CSS `width` declaration for a single column, or `None` for content-sized.
fn col_width_style(width: ColWidth, proportional_total: f32) -> Option<String> {
    match width {
        ColWidth::Fixed(pts) => Some(format!("width:{:.2}pt", pts.value())),
        ColWidth::Proportional(share) if proportional_total > 0.0 => {
            Some(format!("width:{:.2}%", share / proportional_total * 100.0))
        }
        _ => None,
    }
}

/// CSS `width` declaration for the whole table, or `None` for auto.
fn table_width_style(width: Option<TableWidth>) -> Option<String> {
    match width {
        Some(TableWidth::Fixed(pts)) => Some(format!("width:{pts:.2}pt")),
        Some(TableWidth::Percent(p)) => Some(format!("width:{p:.2}%")),
        _ => None,
    }
}

/// Builds the combined `style` value for a cell from its resolved horizontal
/// alignment (cell override → column default) and vertical alignment.
fn cell_style(cell: &Cell, col_spec: Option<&ColSpec>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    let align = if cell.alignment != ColAlignment::Default {
        cell.alignment
    } else {
        col_spec.map(|c| c.alignment).unwrap_or_default()
    };
    if let Some(css) = horizontal_align_css(align) {
        parts.push(format!("text-align:{css}"));
    }

    if let Some(css) = vertical_align_css(cell.props.vertical_align) {
        parts.push(format!("vertical-align:{css}"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(";"))
    }
}

/// Maps a horizontal alignment to its CSS keyword, or `None` for the default.
fn horizontal_align_css(align: ColAlignment) -> Option<&'static str> {
    match align {
        ColAlignment::Left => Some("left"),
        ColAlignment::Right => Some("right"),
        ColAlignment::Center => Some("center"),
        // `Default` and any future variant carry no explicit alignment.
        _ => None,
    }
}

/// Maps a vertical alignment to its CSS keyword, or `None` for the default top.
fn vertical_align_css(align: Option<CellVerticalAlign>) -> Option<&'static str> {
    match align {
        Some(CellVerticalAlign::Middle) => Some("middle"),
        Some(CellVerticalAlign::Bottom) => Some("bottom"),
        // `Top` is the default; `None` and any future variant emit nothing.
        _ => None,
    }
}

#[cfg(test)]
#[path = "tables_tests.rs"]
mod tests;
