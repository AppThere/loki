// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table mapper: `w:tbl` → `Block::Table`.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth, TableWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{
    Cell, CellProps, CellTextDirection, CellVerticalAlign, Row,
};
use loki_primitives::units::Points;

use crate::docx::model::styles::{DocxTableModel, DocxTextDirection, DocxVAlign};

use super::document::MappingContext;
use super::paragraph::map_paragraph;
use super::props::map_border_edge;
use super::table_look::map_tbl_look;

#[path = "table_vmerge.rs"]
mod vmerge;
use vmerge::compute_v_merge_spans;

/// Maps a `w:tbl` to a `Block::Table`. Two passes resolve `w:vMerge` spans
/// (restart cells get `row_span`, continuations dropped — §17.4.84);
/// `w:tblGrid` widths convert twips→points, else `ColWidth::Default`.
pub(crate) fn map_table(t: &DocxTableModel, ctx: &mut MappingContext<'_>) -> Block {
    let col_specs = build_col_specs(t);

    let (span_map, skip_set) = compute_v_merge_spans(&t.rows);

    let mut head_rows: Vec<Row> = Vec::new();
    let mut body_rows: Vec<Row> = Vec::new();

    for (row_idx, tr) in t.rows.iter().enumerate() {
        let is_header = tr.tr_pr.as_ref().is_some_and(|p| p.is_header);

        let mut grid_col: usize = 0;
        let mut cells: Vec<Cell> = Vec::new();
        for (cell_idx, tc) in tr.cells.iter().enumerate() {
            let col_span = tc
                .tc_pr
                .as_ref()
                .and_then(|p| p.grid_span)
                .unwrap_or(1)
                .max(1) as usize;
            // Continuations dropped; the spanning cell carries row_span=N.
            if !skip_set.contains(&(row_idx, cell_idx)) {
                let mut cell = map_cell(tc, ctx);
                cell.row_span = span_map.get(&(row_idx, grid_col)).copied().unwrap_or(1);
                cells.push(cell);
            }
            grid_col += col_span;
        }

        let row = Row::new(cells);
        if is_header {
            head_rows.push(row);
        } else {
            body_rows.push(row);
        }
    }

    let head = if head_rows.is_empty() {
        TableHead::empty()
    } else {
        TableHead {
            attr: NodeAttr::default(),
            rows: head_rows,
        }
    };

    let body = TableBody::from_rows(body_rows);

    let width = map_tbl_width(t);

    let mut attr = NodeAttr::default();
    if t.tbl_pr
        .as_ref()
        .and_then(|p| p.layout.as_deref())
        .is_some_and(|l| l == "fixed")
    {
        attr.classes
            .push(loki_doc_model::content::table::core::TABLE_FIXED_LAYOUT_CLASS.to_string());
    }
    // `w:tblStyle` → the referenced table style, stored in the `"style"` attr.
    if let Some(id) = t.tbl_pr.as_ref().and_then(|p| p.style_id.clone()) {
        attr.kv.push(("style".to_string(), id));
    }
    // `w:tblLook` → active conditional-style regions, encoded in `"tbllook"`.
    if let Some(l) = t.tbl_pr.as_ref().and_then(|p| p.tbl_look) {
        attr.kv.push(("tbllook".to_string(), map_tbl_look(l)));
    }

    let table = Table {
        attr,
        caption: TableCaption::default(),
        width,
        col_specs,
        head,
        bodies: vec![body],
        foot: TableFoot::empty(),
    };

    Block::Table(Box::new(table))
}

/// Converts `w:tblW` to [`TableWidth`]. COMPAT(microsoft): @w:type="pct" is
/// fiftieths of a percent — divide by 50 for the 0.0–100.0 range.
#[allow(clippy::cast_precision_loss)] // twip values are small; f32 precision is sufficient
fn map_tbl_width(t: &DocxTableModel) -> Option<TableWidth> {
    let w = t.tbl_pr.as_ref()?.width.as_ref()?;
    Some(match w.w_type.as_str() {
        "dxa" => TableWidth::Fixed(w.w as f32 / 20.0),
        "pct" => TableWidth::Percent(w.w as f32 / 50.0),
        _ => TableWidth::Auto, // "auto" | "nil" | unknown
    })
}

/// Builds column specifications from `w:tblGrid` column widths.
fn build_col_specs(t: &DocxTableModel) -> Vec<ColSpec> {
    if t.col_widths.is_empty() {
        // Fall back: infer column count from the widest row.
        let num_cols = t.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
        (0..num_cols)
            .map(|_| ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            })
            .collect()
    } else {
        t.col_widths
            .iter()
            .map(|&w| ColSpec {
                alignment: ColAlignment::Default,
                width: if w > 0 {
                    ColWidth::Fixed(Points::new(f64::from(w) / 20.0))
                } else {
                    ColWidth::Default
                },
            })
            .collect()
    }
}

/// Maps a `w:tc` table cell.
fn map_cell(tc: &crate::docx::model::styles::DocxTableCell, ctx: &mut MappingContext<'_>) -> Cell {
    use crate::docx::model::document::DocxBodyChild;
    let col_span = tc.tc_pr.as_ref().and_then(|p| p.grid_span).unwrap_or(1);
    // Ordered cell content: paragraphs + nested tables (recursing map_table).
    let blocks: Vec<Block> = tc
        .children
        .iter()
        .flat_map(|child| match child {
            DocxBodyChild::Paragraph(p) => map_paragraph(p, ctx),
            DocxBodyChild::Table(t) => vec![map_table(t, ctx)],
        })
        .collect();

    let mut props = CellProps::default();
    // w:cnfStyle mask (4a.3) rides the cell attr for the shading resolver.
    let cnf = tc
        .tc_pr
        .as_ref()
        .and_then(|p| p.cnf_style.clone())
        .filter(|c| loki_doc_model::style::table_cnf::TableCnf::decode_attr(c).is_some());
    if let Some(tc_pr) = tc.tc_pr.as_ref() {
        // `w:shd` background; `pctN` patterns blend @w:color over @w:fill.
        if let Some(rgb) = crate::xml_util::resolve_shading(
            tc_pr.shd_fill.as_deref(),
            tc_pr.shd_val.as_deref(),
            tc_pr.shd_color.as_deref(),
        ) {
            use loki_primitives::color::DocumentColor;
            props.background_color = Some(DocumentColor::Rgb(rgb));
        }
        // Cell borders from `w:tcBorders`.
        if let Some(ref borders) = tc_pr.tc_borders {
            props.border_top = borders.top.as_ref().map(map_border_edge);
            props.border_bottom = borders.bottom.as_ref().map(map_border_edge);
            props.border_left = borders.left.as_ref().map(map_border_edge);
            props.border_right = borders.right.as_ref().map(map_border_edge);
        }
        // Cell padding from `w:tcMar`. COMPAT(ooxml-dxa): twips ÷ 20 = points.
        if let Some(ref m) = tc_pr.tc_margins {
            props.padding_top = m.top.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_bottom = m.bottom.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_left = m.left.map(|v| Points::new(f64::from(v) / 20.0));
            props.padding_right = m.right.map(|v| Points::new(f64::from(v) / 20.0));
        }
        // Vertical alignment from `w:vAlign`.
        props.vertical_align = tc_pr.v_align.map(|v| match v {
            DocxVAlign::Top => CellVerticalAlign::Top,
            DocxVAlign::Center => CellVerticalAlign::Middle,
            DocxVAlign::Bottom => CellVerticalAlign::Bottom,
        });
        // Text direction from `w:textDirection`.
        props.text_direction = tc_pr.text_direction.map(|d| match d {
            DocxTextDirection::LrTb => CellTextDirection::LrTb,
            DocxTextDirection::TbRl => CellTextDirection::TbRl,
            DocxTextDirection::TbLr => CellTextDirection::TbLr,
            DocxTextDirection::BtLr => CellTextDirection::BtLr,
        });
    }

    let mut cell = Cell {
        attr: NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span: 1, // overridden by map_table after compute_v_merge_spans
        col_span,
        blocks,
        props,
    };
    cell.set_cnf_code(cnf);
    cell
}

// ── vMerge two-pass algorithm ─────────────────────────────────────────────────

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "table_tests.rs"]
mod tests;
