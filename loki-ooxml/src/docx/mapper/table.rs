// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Table mapper: `w:tbl` → `Block::Table`.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, CellProps, Row};
use loki_primitives::units::Points;

use crate::docx::model::styles::DocxTableModel;

use super::document::MappingContext;
use super::paragraph::map_paragraph;
use super::props::map_border_edge;

/// Maps a `w:tbl` to a `Block::Table`.
///
/// ## Limitations (v0.1.0)
/// - Vertical merge spans are not tracked; all cells report `row_span = 1`.
/// - Column widths from `w:tblGrid` are converted from twips to points when
///   present; otherwise `ColWidth::Default` is used.
/// - Table borders and shading are not yet mapped.
pub(crate) fn map_table(t: &DocxTableModel, ctx: &mut MappingContext<'_>) -> Block {
    // Build column specifications from tblGrid widths.
    let col_specs = build_col_specs(t);

    // Partition rows into header and body.
    let mut head_rows: Vec<Row> = Vec::new();
    let mut body_rows: Vec<Row> = Vec::new();

    for tr in &t.rows {
        let is_header = tr.tr_pr.as_ref().map(|p| p.is_header).unwrap_or(false);
        let cells: Vec<Cell> = tr.cells.iter().map(|tc| map_cell(tc, ctx)).collect();
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
        TableHead { attr: NodeAttr::default(), rows: head_rows }
    };

    let body = TableBody::from_rows(body_rows);

    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        col_specs,
        head,
        bodies: vec![body],
        foot: TableFoot::empty(),
    };

    Block::Table(Box::new(table))
}

/// Builds column specifications from `w:tblGrid` column widths.
fn build_col_specs(t: &DocxTableModel) -> Vec<ColSpec> {
    if t.col_widths.is_empty() {
        // Fall back: infer column count from the widest row.
        let num_cols = t.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
        (0..num_cols)
            .map(|_| ColSpec { alignment: ColAlignment::Default, width: ColWidth::Default })
            .collect()
    } else {
        t.col_widths
            .iter()
            .map(|&w| ColSpec {
                alignment: ColAlignment::Default,
                width: if w > 0 {
                    ColWidth::Fixed(Points::new(w as f64 / 20.0))
                } else {
                    ColWidth::Default
                },
            })
            .collect()
    }
}

/// Maps a `w:tc` table cell.
fn map_cell(
    tc: &crate::docx::model::styles::DocxTableCell,
    ctx: &mut MappingContext<'_>,
) -> Cell {
    let col_span = tc.tc_pr.as_ref().and_then(|p| p.grid_span).unwrap_or(1);
    let blocks: Vec<Block> = tc.paragraphs.iter()
        .flat_map(|p| map_paragraph(p, ctx))
        .collect();

    let mut props = CellProps::default();
    if let Some(tc_pr) = tc.tc_pr.as_ref() {
        // Cell background from `w:shd @w:fill`.
        if let Some(ref hex) = tc_pr.shd_fill {
            if let Some(rgb) = crate::xml_util::hex_color(hex) {
                use loki_primitives::color::DocumentColor;
                props.background_color = Some(DocumentColor::Rgb(rgb));
            }
        }
        // Cell borders from `w:tcBorders`.
        if let Some(ref borders) = tc_pr.tc_borders {
            props.border_top = borders.top.as_ref().map(map_border_edge);
            props.border_bottom = borders.bottom.as_ref().map(map_border_edge);
            props.border_left = borders.left.as_ref().map(map_border_edge);
            props.border_right = borders.right.as_ref().map(map_border_edge);
        }
    }

    Cell {
        attr: NodeAttr::default(),
        alignment: ColAlignment::Default,
        row_span: 1, // v0.1.0: vMerge tracking not yet implemented
        col_span,
        blocks,
        props,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use loki_doc_model::content::block::Block;
    use loki_doc_model::style::catalog::StyleCatalog;
    use loki_opc::PartData;
    use crate::docx::import::DocxImportOptions;
    use crate::docx::model::paragraph::DocxParagraph;
    use crate::docx::model::styles::{DocxTableCell, DocxTableRow, DocxTcPr, DocxTrPr};

    fn make_ctx<'a>(
        styles: &'a StyleCatalog,
        footnotes: &'a HashMap<i32, Vec<Block>>,
        endnotes: &'a HashMap<i32, Vec<Block>>,
        hyperlinks: &'a HashMap<String, String>,
        images: &'a HashMap<String, PartData>,
        options: &'a DocxImportOptions,
    ) -> MappingContext<'a> {
        MappingContext { styles, footnotes, endnotes, hyperlinks, images, options, warnings: Vec::new() }
    }

    fn simple_cell(paragraphs: Vec<DocxParagraph>) -> DocxTableCell {
        DocxTableCell { tc_pr: None, paragraphs }
    }

    fn simple_row(cells: Vec<DocxTableCell>) -> DocxTableRow {
        DocxTableRow { tr_pr: None, cells }
    }

    #[test]
    fn empty_table_produces_table_block() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let t = DocxTableModel { tbl_pr: None, col_widths: vec![], rows: vec![] };
        let block = map_table(&t, &mut ctx);
        assert!(matches!(block, Block::Table(_)));
        if let Block::Table(tbl) = block {
            assert_eq!(tbl.col_specs.len(), 0);
            assert!(tbl.bodies[0].body_rows.is_empty());
        }
    }

    #[test]
    fn two_by_two_table() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let t = DocxTableModel {
            tbl_pr: None,
            col_widths: vec![1440, 1440], // 72pt each
            rows: vec![
                simple_row(vec![simple_cell(vec![DocxParagraph::default()]),
                                 simple_cell(vec![DocxParagraph::default()])]),
                simple_row(vec![simple_cell(vec![DocxParagraph::default()]),
                                 simple_cell(vec![DocxParagraph::default()])]),
            ],
        };
        let block = map_table(&t, &mut ctx);
        if let Block::Table(tbl) = block {
            assert_eq!(tbl.col_specs.len(), 2);
            assert_eq!(tbl.bodies[0].body_rows.len(), 2);
            assert_eq!(tbl.bodies[0].body_rows[0].cells.len(), 2);
            // 1440 twips = 72 pt
            assert!(matches!(tbl.col_specs[0].width, ColWidth::Fixed(p) if (p.value() - 72.0).abs() < 0.01));
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn header_row_goes_to_head() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let header_row = DocxTableRow {
            tr_pr: Some(DocxTrPr { is_header: true }),
            cells: vec![simple_cell(vec![])],
        };
        let body_row = simple_row(vec![simple_cell(vec![])]);
        let t = DocxTableModel {
            tbl_pr: None,
            col_widths: vec![],
            rows: vec![header_row, body_row],
        };
        let block = map_table(&t, &mut ctx);
        if let Block::Table(tbl) = block {
            assert_eq!(tbl.head.rows.len(), 1);
            assert_eq!(tbl.bodies[0].body_rows.len(), 1);
        } else {
            panic!("expected Table");
        }
    }

    #[test]
    fn cell_col_span_preserved() {
        let styles = StyleCatalog::default();
        let (fn_m, en_m, hl_m, img_m) = (HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new());
        let opts = DocxImportOptions::default();
        let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

        let cell_with_span = DocxTableCell {
            tc_pr: Some(DocxTcPr { grid_span: Some(3), v_merge: None, ..Default::default() }),
            paragraphs: vec![],
        };
        let t = DocxTableModel {
            tbl_pr: None,
            col_widths: vec![],
            rows: vec![simple_row(vec![cell_with_span])],
        };
        let block = map_table(&t, &mut ctx);
        if let Block::Table(tbl) = block {
            assert_eq!(tbl.bodies[0].body_rows[0].cells[0].col_span, 3);
        } else {
            panic!("expected Table");
        }
    }
}
