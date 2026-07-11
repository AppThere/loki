// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `table`.

use super::*;
use crate::docx::import::DocxImportOptions;
use crate::docx::model::document::DocxBodyChild;
use crate::docx::model::paragraph::DocxParagraph;
use crate::docx::model::styles::{DocxTableCell, DocxTableRow, DocxTcPr, DocxTrPr};
use loki_doc_model::content::block::Block;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_opc::PartData;
use std::collections::HashMap;

fn make_ctx<'a>(
    styles: &'a StyleCatalog,
    footnotes: &'a HashMap<i32, Vec<Block>>,
    endnotes: &'a HashMap<i32, Vec<Block>>,
    hyperlinks: &'a HashMap<String, String>,
    images: &'a HashMap<String, PartData>,
    options: &'a DocxImportOptions,
) -> MappingContext<'a> {
    MappingContext {
        styles,
        footnotes,
        endnotes,
        hyperlinks,
        images,
        options,
        warnings: Vec::new(),
        open_bookmarks: Vec::new(),
    }
}

fn simple_cell(paragraphs: Vec<DocxParagraph>) -> DocxTableCell {
    DocxTableCell {
        tc_pr: None,
        children: paragraphs
            .into_iter()
            .map(DocxBodyChild::Paragraph)
            .collect(),
    }
}

fn simple_row(cells: Vec<DocxTableCell>) -> DocxTableRow {
    DocxTableRow { tr_pr: None, cells }
}

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
fn nested_table_in_cell_maps_to_table_block() {
    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    // Inner 1×1 table nested inside the outer cell, after a paragraph.
    let inner = DocxTableModel {
        tbl_pr: None,
        col_widths: vec![1440],
        rows: vec![simple_row(vec![simple_cell(
            vec![DocxParagraph::default()],
        )])],
    };
    let outer_cell = DocxTableCell {
        tc_pr: None,
        children: vec![
            DocxBodyChild::Paragraph(DocxParagraph::default()),
            DocxBodyChild::Table(inner),
        ],
    };
    let t = DocxTableModel {
        tbl_pr: None,
        col_widths: vec![1440],
        rows: vec![simple_row(vec![outer_cell])],
    };
    let block = map_table(&t, &mut ctx);
    let Block::Table(tbl) = block else {
        panic!("expected outer Table");
    };
    let cell = &tbl.bodies[0].body_rows[0].cells[0];
    // The cell preserves order: a paragraph then a nested Table block.
    assert!(
        cell.blocks.len() >= 2,
        "cell should hold the paragraph and the nested table"
    );
    assert!(
        cell.blocks.iter().any(|b| matches!(b, Block::Table(_))),
        "nested w:tbl inside w:tc must map to a Block::Table in the cell"
    );
}

#[test]
fn tbl_layout_fixed_marks_table_class() {
    use crate::docx::model::styles::DocxTblPr;
    use loki_doc_model::content::table::core::TABLE_FIXED_LAYOUT_CLASS;

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
        tbl_pr: Some(DocxTblPr {
            layout: Some("fixed".to_string()),
            ..Default::default()
        }),
        col_widths: vec![1440, 1440],
        rows: vec![simple_row(vec![simple_cell(
            vec![DocxParagraph::default()],
        )])],
    };
    let block = map_table(&t, &mut ctx);
    if let Block::Table(tbl) = block {
        assert!(
            tbl.attr
                .classes
                .iter()
                .any(|c| c == TABLE_FIXED_LAYOUT_CLASS),
            "fixed tblLayout must add the fixed-layout class"
        );
    } else {
        panic!("expected Table");
    }
}

#[test]
fn tbl_layout_autofit_has_no_fixed_class() {
    use crate::docx::model::styles::DocxTblPr;
    use loki_doc_model::content::table::core::TABLE_FIXED_LAYOUT_CLASS;

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
        tbl_pr: Some(DocxTblPr {
            layout: Some("autofit".to_string()),
            ..Default::default()
        }),
        col_widths: vec![1440],
        rows: vec![simple_row(vec![simple_cell(
            vec![DocxParagraph::default()],
        )])],
    };
    let block = map_table(&t, &mut ctx);
    if let Block::Table(tbl) = block {
        assert!(
            !tbl.attr
                .classes
                .iter()
                .any(|c| c == TABLE_FIXED_LAYOUT_CLASS),
            "autofit tblLayout must NOT add the fixed-layout class"
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
        children: vec![],
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

// ── compute_v_merge_spans unit tests ─────────────────────────────────────

fn merge_cell(v_merge: DocxVMerge) -> DocxTableCell {
    DocxTableCell {
        tc_pr: Some(DocxTcPr {
            v_merge: Some(v_merge),
            ..Default::default()
        }),
        children: vec![],
    }
}

fn merge_cell_with_col_span(v_merge: DocxVMerge, col_span: u32) -> DocxTableCell {
    DocxTableCell {
        tc_pr: Some(DocxTcPr {
            v_merge: Some(v_merge),
            grid_span: Some(col_span),
            ..Default::default()
        }),
        children: vec![],
    }
}

/// Simple 2-row merge: 2×2 table, col 0 merged across rows 0-1.
#[test]
fn vmerge_simple_2_row_merge() {
    let rows = vec![
        simple_row(vec![merge_cell(DocxVMerge::Restart), simple_cell(vec![])]),
        simple_row(vec![merge_cell(DocxVMerge::Continue), simple_cell(vec![])]),
    ];
    let (span_map, skip_set) = compute_v_merge_spans(&rows);

    assert_eq!(span_map[&(0, 0)], 2, "restart cell should have row_span=2");
    assert!(
        skip_set.contains(&(1, 0)),
        "continuation cell (1,0) should be skipped"
    );
    assert!(
        !skip_set.contains(&(0, 0)),
        "restart cell must not be skipped"
    );
    assert!(
        !skip_set.contains(&(0, 1)),
        "col-1 cells must not be skipped"
    );
    assert!(
        !skip_set.contains(&(1, 1)),
        "col-1 cells must not be skipped"
    );
}

/// 3-row merge: col 0 merged across 3 rows.
#[test]
fn vmerge_3_row_merge() {
    let rows = vec![
        simple_row(vec![merge_cell(DocxVMerge::Restart), simple_cell(vec![])]),
        simple_row(vec![merge_cell(DocxVMerge::Continue), simple_cell(vec![])]),
        simple_row(vec![merge_cell(DocxVMerge::Continue), simple_cell(vec![])]),
    ];
    let (span_map, skip_set) = compute_v_merge_spans(&rows);

    assert_eq!(span_map[&(0, 0)], 3, "restart cell should have row_span=3");
    assert!(
        skip_set.contains(&(1, 0)),
        "row 1 continuation must be skipped"
    );
    assert!(
        skip_set.contains(&(2, 0)),
        "row 2 continuation must be skipped"
    );
}

/// No merge: table with no vMerge → all cells `row_span=1`, none removed.
#[test]
fn vmerge_no_merge() {
    let rows = vec![
        simple_row(vec![simple_cell(vec![]), simple_cell(vec![])]),
        simple_row(vec![simple_cell(vec![]), simple_cell(vec![])]),
    ];
    let (span_map, skip_set) = compute_v_merge_spans(&rows);

    assert!(span_map.is_empty(), "no spans expected");
    assert!(skip_set.is_empty(), "no cells to skip");
}

/// Multiple independent merges in different columns.
#[test]
fn vmerge_multiple_independent_merges() {
    // 3×2 table: col 0 merged rows 0-1, col 1 merged rows 1-2.
    let rows = vec![
        simple_row(vec![merge_cell(DocxVMerge::Restart), simple_cell(vec![])]),
        simple_row(vec![
            merge_cell(DocxVMerge::Continue),
            merge_cell(DocxVMerge::Restart),
        ]),
        simple_row(vec![simple_cell(vec![]), merge_cell(DocxVMerge::Continue)]),
    ];
    let (span_map, skip_set) = compute_v_merge_spans(&rows);

    assert_eq!(span_map[&(0, 0)], 2, "col-0 restart at row 0 → span 2");
    assert_eq!(span_map[&(1, 1)], 2, "col-1 restart at row 1 → span 2");
    assert!(
        skip_set.contains(&(1, 0)),
        "col-0 continuation (row 1) skipped"
    );
    assert!(
        skip_set.contains(&(2, 1)),
        "col-1 continuation (row 2) skipped"
    );
    assert!(!skip_set.contains(&(0, 1)), "col-1 row 0 is a plain cell");
    assert!(!skip_set.contains(&(2, 0)), "col-0 row 2 is a plain cell");
}

fn cell_with_props(tc_pr: DocxTcPr) -> DocxTableCell {
    DocxTableCell {
        tc_pr: Some(tc_pr),
        children: vec![],
    }
}

#[test]
fn cell_padding_maps_to_points() {
    use crate::docx::model::styles::DocxCellMargins;
    use loki_primitives::units::Points;

    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let tc = cell_with_props(DocxTcPr {
        tc_margins: Some(DocxCellMargins {
            top: Some(100),    // 5pt
            bottom: Some(200), // 10pt
            left: Some(300),   // 15pt
            right: Some(400),  // 20pt
        }),
        ..Default::default()
    });
    let cell = map_cell(&tc, &mut ctx);
    assert_eq!(cell.props.padding_top, Some(Points::new(5.0)));
    assert_eq!(cell.props.padding_bottom, Some(Points::new(10.0)));
    assert_eq!(cell.props.padding_left, Some(Points::new(15.0)));
    assert_eq!(cell.props.padding_right, Some(Points::new(20.0)));
}

#[test]
fn cell_valign_maps_correctly() {
    use crate::docx::model::styles::DocxVAlign;
    use loki_doc_model::content::table::row::CellVerticalAlign;

    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    for (docx_val, expected) in [
        (DocxVAlign::Top, CellVerticalAlign::Top),
        (DocxVAlign::Center, CellVerticalAlign::Middle),
        (DocxVAlign::Bottom, CellVerticalAlign::Bottom),
    ] {
        let tc = cell_with_props(DocxTcPr {
            v_align: Some(docx_val),
            ..Default::default()
        });
        let cell = map_cell(&tc, &mut ctx);
        assert_eq!(cell.props.vertical_align, Some(expected));
    }
}

#[test]
fn cell_text_direction_maps_correctly() {
    use crate::docx::model::styles::DocxTextDirection;
    use loki_doc_model::content::table::row::CellTextDirection;

    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    for (docx_val, expected) in [
        (DocxTextDirection::LrTb, CellTextDirection::LrTb),
        (DocxTextDirection::TbRl, CellTextDirection::TbRl),
        (DocxTextDirection::TbLr, CellTextDirection::TbLr),
        (DocxTextDirection::BtLr, CellTextDirection::BtLr),
    ] {
        let tc = cell_with_props(DocxTcPr {
            text_direction: Some(docx_val),
            ..Default::default()
        });
        let cell = map_cell(&tc, &mut ctx);
        assert_eq!(cell.props.text_direction, Some(expected));
    }
}

/// `col_span` + vMerge: a restart cell with `col_span=2` spans two grid columns.
#[test]
fn vmerge_with_col_span() {
    // 2×1 logical table: row 0 has a 2-wide restart, row 1 has a 2-wide continuation.
    let rows = vec![
        simple_row(vec![merge_cell_with_col_span(DocxVMerge::Restart, 2)]),
        simple_row(vec![merge_cell_with_col_span(DocxVMerge::Continue, 2)]),
    ];
    let (span_map, skip_set) = compute_v_merge_spans(&rows);

    // Grid col 0 (first of the two expanded columns) holds the span.
    assert_eq!(
        span_map[&(0, 0)],
        2,
        "wide restart cell should have row_span=2"
    );
    assert!(
        skip_set.contains(&(1, 0)),
        "wide continuation cell must be skipped"
    );
}
