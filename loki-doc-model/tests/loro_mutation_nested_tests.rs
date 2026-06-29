// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Nested-addressing mutations: editing text inside table cells and
//! footnote/endnote bodies via a [`BlockPath`] (including a recursive
//! cell → note path), proving the live containers are reachable and that edits
//! round-trip through `loro_to_document` (the bridge rebuilds each cell / note
//! body from the same containers).

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::loro_schema::MARK_BOLD;
use loki_doc_model::{
    BlockPath, MutationError, PathStep, delete_text_at, get_block_text_at, insert_text_at,
    mark_text_at,
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

// ── Note-body addressing ────────────────────────────────────────────────────

fn note_para(text: &str, note_bodies: Vec<(&str, &str)>) -> Block {
    // A paragraph: leading text, then one footnote per (refless, body) pair.
    let mut inlines = vec![Inline::Str(text.into())];
    for (_, body) in note_bodies {
        inlines.push(Inline::Note(
            NoteKind::Footnote,
            vec![Block::Para(vec![Inline::Str(body.into())])],
        ));
    }
    Block::Para(inlines)
}

/// Plain text of the `note_ord`-th note's first body paragraph in block
/// `para_block` of a rebuilt document.
fn note_body_text(doc: &Document, para_block: usize, note_ord: usize) -> String {
    let Block::Para(inlines) = &doc.sections[0].blocks[para_block] else {
        panic!("para");
    };
    let bodies: Vec<&Vec<Block>> = inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Note(_, body) => Some(body),
            _ => None,
        })
        .collect();
    let Block::Para(binlines) = &bodies[note_ord][0] else {
        panic!("body para");
    };
    binlines
        .iter()
        .filter_map(|i| match i {
            Inline::Str(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn reads_text_inside_a_note_body() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![note_para("ref", vec![("", "body")])];
    let loro = document_to_loro(&doc).unwrap();
    assert_eq!(
        get_block_text_at(&loro, &BlockPath::in_note(0, 0, 0)),
        "body"
    );
}

#[test]
fn edits_text_inside_a_note_body_and_round_trips() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![note_para("ref", vec![("", "body")])];
    let loro = document_to_loro(&doc).unwrap();
    insert_text_at(&loro, &BlockPath::in_note(0, 0, 0), 4, "!").unwrap(); // "body" -> "body!"
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(note_body_text(&rebuilt, 0, 0), "body!");
}

#[test]
fn addresses_the_correct_note_among_several() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![note_para("x", vec![("", "first"), ("", "second")])];
    let loro = document_to_loro(&doc).unwrap();
    // Edit the *second* note's body only.
    insert_text_at(&loro, &BlockPath::in_note(0, 1, 0), 6, "!").unwrap();
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(note_body_text(&rebuilt, 0, 0), "first");
    assert_eq!(note_body_text(&rebuilt, 0, 1), "second!");
}

#[test]
fn descending_into_a_block_without_notes_errors() {
    let loro = document_to_loro(&doc_with_table()).unwrap();
    // Block 0 is a plain paragraph with no notes container.
    let err = insert_text_at(&loro, &BlockPath::in_note(0, 0, 0), 0, "x");
    assert!(matches!(err, Err(MutationError::InvalidBlockPath(_))));
}

#[test]
fn edits_a_note_nested_inside_a_table_cell() {
    // A table whose single cell holds a paragraph containing a footnote — the
    // path descends Cell then Note, proving recursive container addressing.
    let cell = Cell::simple(vec![Block::Para(vec![
        Inline::Str("c".into()),
        Inline::Note(
            NoteKind::Footnote,
            vec![Block::Para(vec![Inline::Str("fn".into())])],
        ),
    ])]);
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![cell])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];
    let loro = document_to_loro(&doc).unwrap();

    let path = BlockPath {
        root: 0,
        steps: vec![
            PathStep::Cell { cell: 0, block: 0 },
            PathStep::Note { note: 0, block: 0 },
        ],
    };
    assert_eq!(get_block_text_at(&loro, &path), "fn");
    insert_text_at(&loro, &path, 2, "!").unwrap(); // "fn" -> "fn!"

    let rebuilt = loro_to_document(&loro).unwrap();
    let Block::Table(t) = &rebuilt.sections[0].blocks[0] else {
        panic!("table");
    };
    let Block::Para(cell_inlines) = &t.bodies[0].body_rows[0].cells[0].blocks[0] else {
        panic!("cell para");
    };
    let body = cell_inlines
        .iter()
        .find_map(|i| match i {
            Inline::Note(_, b) => Some(b),
            _ => None,
        })
        .expect("note in cell");
    let Block::Para(binlines) = &body[0] else {
        panic!("note body para");
    };
    let text: String = binlines
        .iter()
        .filter_map(|i| match i {
            Inline::Str(s) => Some(s.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, "fn!");
}
