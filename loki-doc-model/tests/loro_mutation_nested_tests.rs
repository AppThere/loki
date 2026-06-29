// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Nested-addressing mutations: editing text inside table cells via a
//! [`BlockPath`], proving the live cell containers are reachable and that edits
//! round-trip through `loro_to_document` (the bridge rebuilds each cell from the
//! same containers).

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::loro_schema::MARK_BOLD;
use loki_doc_model::{
    BlockPath, MutationError, delete_text_at, get_block_text_at, insert_text_at, mark_text_at,
};
use loro::LoroValue;

fn text_cell(s: &str) -> Cell {
    Cell::simple(vec![Block::Para(vec![Inline::Str(s.into())])])
}

/// `[Para("intro"), Table]` — the table is global block index 1, with one body
/// row of two cells "a" | "b" (flat cell indices 0 and 1).
fn doc_with_table() -> Document {
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![
            text_cell("a"),
            text_cell("b"),
        ])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![
        Block::Para(vec![Inline::Str("intro".into())]),
        Block::Table(Box::new(table)),
    ];
    doc
}

/// Returns the first paragraph's text in the `cell`-th cell of the table at
/// global block index 1 of a rebuilt document.
fn cell_para_text(doc: &Document, cell: usize) -> String {
    let Block::Table(t) = &doc.sections[0].blocks[1] else {
        panic!("expected a table");
    };
    let cells = &t.bodies[0].body_rows[0].cells;
    let Block::Para(inlines) = &cells[cell].blocks[0] else {
        panic!("expected a paragraph");
    };
    inlines
        .iter()
        .map(|i| match i {
            Inline::Str(s) => s.as_str(),
            _ => "",
        })
        .collect()
}

#[test]
fn reads_text_inside_a_table_cell() {
    let loro = document_to_loro(&doc_with_table()).unwrap();
    assert_eq!(get_block_text_at(&loro, &BlockPath::in_cell(1, 0, 0)), "a");
    assert_eq!(get_block_text_at(&loro, &BlockPath::in_cell(1, 1, 0)), "b");
}

#[test]
fn inserts_text_inside_a_table_cell_and_round_trips() {
    let loro = document_to_loro(&doc_with_table()).unwrap();
    // Append to the second cell ("b" -> "bX"); first cell untouched.
    insert_text_at(&loro, &BlockPath::in_cell(1, 1, 0), 1, "X").unwrap();
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(cell_para_text(&rebuilt, 1), "bX");
    assert_eq!(cell_para_text(&rebuilt, 0), "a");
}

#[test]
fn deletes_text_inside_a_table_cell() {
    let loro = document_to_loro(&doc_with_table()).unwrap();
    insert_text_at(&loro, &BlockPath::in_cell(1, 0, 0), 1, "bcd").unwrap(); // "a" -> "abcd"
    delete_text_at(&loro, &BlockPath::in_cell(1, 0, 0), 1, 2).unwrap(); // -> "ad"
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(cell_para_text(&rebuilt, 0), "ad");
}

#[test]
fn marks_text_inside_a_table_cell() {
    let loro = document_to_loro(&doc_with_table()).unwrap();
    mark_text_at(
        &loro,
        &BlockPath::in_cell(1, 0, 0),
        0,
        1,
        MARK_BOLD,
        LoroValue::Bool(true),
    )
    .unwrap();
    let rebuilt = loro_to_document(&loro).unwrap();
    let Block::Table(t) = &rebuilt.sections[0].blocks[1] else {
        panic!("table");
    };
    let Block::Para(inlines) = &t.bodies[0].body_rows[0].cells[0].blocks[0] else {
        panic!("para");
    };
    let bold = inlines.iter().any(|i| {
        matches!(i, Inline::StyledRun(r)
            if r.direct_props.as_ref().is_some_and(|p| p.bold == Some(true)))
    });
    assert!(bold, "cell text should be bold: {inlines:?}");
}

#[test]
fn flat_path_matches_the_flat_api() {
    let loro = document_to_loro(&doc_with_table()).unwrap();
    // Root-only path edits the top-level "intro" paragraph (global block 0).
    insert_text_at(&loro, &BlockPath::block(0), 5, "!").unwrap();
    let rebuilt = loro_to_document(&loro).unwrap();
    let Block::Para(inlines) = &rebuilt.sections[0].blocks[0] else {
        panic!("para");
    };
    assert_eq!(
        inlines
            .iter()
            .filter_map(|i| match i {
                Inline::Str(s) => Some(s.as_str()),
                _ => None,
            })
            .collect::<String>(),
        "intro!"
    );
}

#[test]
fn descending_into_a_non_table_block_errors() {
    let loro = document_to_loro(&doc_with_table()).unwrap();
    // Block 0 is a paragraph, not a table.
    let err = insert_text_at(&loro, &BlockPath::in_cell(0, 0, 0), 0, "x");
    assert!(matches!(err, Err(MutationError::InvalidBlockPath(_))));
}

#[test]
fn out_of_range_cell_errors() {
    let loro = document_to_loro(&doc_with_table()).unwrap();
    let err = insert_text_at(&loro, &BlockPath::in_cell(1, 9, 0), 0, "x");
    assert!(matches!(err, Err(MutationError::InvalidBlockPath(_))));
}
