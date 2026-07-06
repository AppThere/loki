// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for the structural table mutations (insert/delete
//! row/column) — plan 4a.2 follow-on.
//!
//! Each test builds a grid table, seeds cell text, applies a mutation, and
//! re-derives the document to assert both the new shape **and** that surviving
//! cells kept their text (the mutation patches the live cell list rather than
//! rebuilding it).

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::{
    MutationError, delete_table_column, delete_table_row, insert_table_column, insert_table_row,
    table_grid_dims,
};
use loro::LoroDoc;

/// A live doc whose only block (index 0) is a `rows`×`cols` grid table with each
/// cell's paragraph seeded to `"r{row}c{col}"`.
fn doc_with_grid(rows: usize, cols: usize) -> LoroDoc {
    let mut table = Table::grid(rows, cols);
    for (r, row) in table.bodies[0].body_rows.iter_mut().enumerate() {
        for (c, cell) in row.cells.iter_mut().enumerate() {
            cell.blocks = vec![Block::Para(vec![Inline::Str(format!("r{r}c{c}").into())])];
        }
    }
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];
    document_to_loro(&doc).expect("document_to_loro")
}

/// The grid of cell texts, row-major, from the re-derived document.
fn grid_texts(loro: &LoroDoc) -> Vec<Vec<String>> {
    let doc = loro_to_document(loro).expect("rebuild");
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("block 0 is not a table");
    };
    t.bodies[0]
        .body_rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| match cell.blocks.first() {
                    Some(Block::Para(inlines)) => inlines
                        .iter()
                        .filter_map(|i| match i {
                            Inline::Str(s) => Some(s.as_str()),
                            _ => None,
                        })
                        .collect(),
                    _ => String::new(),
                })
                .collect()
        })
        .collect()
}

#[test]
fn grid_dims_reports_rows_and_cols() {
    let loro = doc_with_grid(2, 3);
    assert_eq!(table_grid_dims(&loro, 0), Some((2, 3)));
}

#[test]
fn insert_row_below_adds_an_empty_row_and_keeps_text() {
    let loro = doc_with_grid(2, 2);
    // Insert below row 0 → new empty row at index 1.
    insert_table_row(&loro, 0, 1).expect("insert row");
    assert_eq!(table_grid_dims(&loro, 0), Some((3, 2)));
    assert_eq!(
        grid_texts(&loro),
        vec![vec!["r0c0", "r0c1"], vec!["", ""], vec!["r1c0", "r1c1"],],
    );
}

#[test]
fn insert_row_at_end_appends() {
    let loro = doc_with_grid(2, 2);
    insert_table_row(&loro, 0, 2).expect("append row");
    let g = grid_texts(&loro);
    assert_eq!(g.len(), 3);
    assert_eq!(g[2], vec!["", ""], "appended row is empty");
    assert_eq!(g[0], vec!["r0c0", "r0c1"]);
}

#[test]
fn delete_row_removes_it_and_shifts_the_rest() {
    let loro = doc_with_grid(3, 2);
    delete_table_row(&loro, 0, 1).expect("delete middle row");
    assert_eq!(table_grid_dims(&loro, 0), Some((2, 2)));
    assert_eq!(
        grid_texts(&loro),
        vec![vec!["r0c0", "r0c1"], vec!["r2c0", "r2c1"]],
    );
}

#[test]
fn delete_last_remaining_row_is_refused() {
    let loro = doc_with_grid(1, 2);
    let err = delete_table_row(&loro, 0, 0).expect_err("must refuse");
    assert!(matches!(err, MutationError::UnsupportedTableStructure(_)));
    // Table is untouched.
    assert_eq!(table_grid_dims(&loro, 0), Some((1, 2)));
}

#[test]
fn insert_column_adds_a_cell_to_every_row_and_keeps_text() {
    let loro = doc_with_grid(2, 2);
    // Insert to the left of column 1.
    insert_table_column(&loro, 0, 1).expect("insert column");
    assert_eq!(table_grid_dims(&loro, 0), Some((2, 3)));
    assert_eq!(
        grid_texts(&loro),
        vec![vec!["r0c0", "", "r0c1"], vec!["r1c0", "", "r1c1"],],
    );
}

#[test]
fn append_column_adds_a_trailing_cell() {
    let loro = doc_with_grid(2, 2);
    insert_table_column(&loro, 0, 2).expect("append column");
    assert_eq!(
        grid_texts(&loro),
        vec![vec!["r0c0", "r0c1", ""], vec!["r1c0", "r1c1", ""],],
    );
}

#[test]
fn delete_column_removes_it_from_every_row() {
    let loro = doc_with_grid(2, 3);
    delete_table_column(&loro, 0, 1).expect("delete middle column");
    assert_eq!(table_grid_dims(&loro, 0), Some((2, 2)));
    assert_eq!(
        grid_texts(&loro),
        vec![vec!["r0c0", "r0c2"], vec!["r1c0", "r1c2"],],
    );
}

#[test]
fn delete_last_remaining_column_is_refused() {
    let loro = doc_with_grid(2, 1);
    let err = delete_table_column(&loro, 0, 0).expect_err("must refuse");
    assert!(matches!(err, MutationError::UnsupportedTableStructure(_)));
    assert_eq!(table_grid_dims(&loro, 0), Some((2, 1)));
}

#[test]
fn out_of_range_indices_error() {
    let loro = doc_with_grid(2, 2);
    assert!(insert_table_row(&loro, 0, 3).is_err(), "row 3 > rows 2");
    assert!(delete_table_row(&loro, 0, 5).is_err());
    assert!(insert_table_column(&loro, 0, 3).is_err(), "col 3 > cols 2");
    assert!(delete_table_column(&loro, 0, 9).is_err());
}

#[test]
fn a_table_with_a_merged_cell_is_rejected() {
    let mut table = Table::grid(2, 2);
    table.bodies[0].body_rows[0].cells[0].col_span = 2;
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];
    let loro = document_to_loro(&doc).expect("to loro");
    assert_eq!(table_grid_dims(&loro, 0), None, "not a simple grid");
    assert!(matches!(
        insert_table_row(&loro, 0, 1),
        Err(MutationError::UnsupportedTableStructure(_))
    ));
}

#[test]
fn edits_compose_across_a_full_round_trip() {
    // A sequence of edits then a save/reload keeps the grid consistent.
    let loro = doc_with_grid(2, 2);
    insert_table_row(&loro, 0, 2).expect("append row"); // 3×2
    insert_table_column(&loro, 0, 0).expect("prepend column"); // 3×3
    delete_table_row(&loro, 0, 0).expect("drop first row"); // 2×3
    assert_eq!(table_grid_dims(&loro, 0), Some((2, 3)));
    assert_eq!(
        grid_texts(&loro),
        vec![vec!["", "r1c0", "r1c1"], vec!["", "", ""],],
    );
}
