// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for `map_table`.

use super::helpers::{make_ctx, simple_cell, simple_row};
use crate::docx::import::DocxImportOptions;
use crate::docx::mapper::table::map_table;
use crate::docx::model::paragraph::DocxParagraph;
use crate::docx::model::styles::{DocxTableCell, DocxTableModel, DocxTableRow, DocxTcPr, DocxTrPr};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::col::ColWidth;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_opc::PartData;
use std::collections::HashMap;

#[test]
fn empty_table_produces_table_block() {
    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let t = DocxTableModel {
        tbl_pr: None,
        col_widths: vec![],
        rows: vec![],
    };
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
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let t = DocxTableModel {
        tbl_pr: None,
        col_widths: vec![1440, 1440], // 72pt each
        rows: vec![
            simple_row(vec![
                simple_cell(vec![DocxParagraph::default()]),
                simple_cell(vec![DocxParagraph::default()]),
            ]),
            simple_row(vec![
                simple_cell(vec![DocxParagraph::default()]),
                simple_cell(vec![DocxParagraph::default()]),
            ]),
        ],
    };
    let block = map_table(&t, &mut ctx);
    if let Block::Table(tbl) = block {
        assert_eq!(tbl.col_specs.len(), 2);
        assert_eq!(tbl.bodies[0].body_rows.len(), 2);
        assert_eq!(tbl.bodies[0].body_rows[0].cells.len(), 2);
        // 1440 twips = 72 pt
        assert!(
            matches!(tbl.col_specs[0].width, ColWidth::Fixed(p) if (p.value() - 72.0).abs() < 0.01)
        );
    } else {
        panic!("expected Table");
    }
}

#[test]
fn header_row_goes_to_head() {
    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
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
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let cell_with_span = DocxTableCell {
        tc_pr: Some(DocxTcPr {
            grid_span: Some(3),
            v_merge: None,
            ..Default::default()
        }),
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
