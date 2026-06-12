// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for `compute_v_merge_spans`.

use super::helpers::{simple_cell, simple_row};
use crate::docx::model::styles::{DocxTableCell, DocxTcPr, DocxVMerge};
use crate::docx::mapper::table::vmerge::compute_v_merge_spans;

fn merge_cell(v_merge: DocxVMerge) -> DocxTableCell {
    DocxTableCell {
        tc_pr: Some(DocxTcPr {
            v_merge: Some(v_merge),
            ..Default::default()
        }),
        paragraphs: vec![],
    }
}

fn merge_cell_with_col_span(v_merge: DocxVMerge, col_span: u32) -> DocxTableCell {
    DocxTableCell {
        tc_pr: Some(DocxTcPr {
            v_merge: Some(v_merge),
            grid_span: Some(col_span),
            ..Default::default()
        }),
        paragraphs: vec![],
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

/// No merge: table with no vMerge → all cells row_span=1, none removed.
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

/// col_span + vMerge: a restart cell with col_span=2 spans two grid columns.
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
