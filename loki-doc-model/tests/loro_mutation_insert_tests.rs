// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Insert primitives for the editor's Insert tab: `insert_block_after`
//! (Insert → Table) and `insert_inline_note_at` (Insert → Footnote). Both write
//! the bridge's own schema against a live document, so the inserted object
//! round-trips through `loro_to_document` and its nested content (table cells /
//! note body) is reachable for editing via a `BlockPath`.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_doc_model::content::table::core::Table;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::{
    BlockPath, PathStep, get_block_text_at, insert_block_after, insert_inline_note_at,
    insert_text_at,
};

fn doc_with_paragraph(text: &str) -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
    doc
}

fn para_text(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Str(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

// ── Table::grid + insert_block_after ────────────────────────────────────────

#[test]
fn grid_builds_evenly_proportioned_empty_cells() {
    let t = Table::grid(2, 3);
    assert_eq!(t.col_count(), 3);
    assert_eq!(t.bodies[0].body_rows.len(), 2);
    assert_eq!(t.bodies[0].body_rows[0].cells.len(), 3);
    // Each cell is a single empty paragraph (immediately editable).
    assert_eq!(
        t.bodies[0].body_rows[0].cells[0].blocks,
        vec![Block::Para(Vec::new())]
    );
}

#[test]
fn grid_clamps_dimensions_to_at_least_one() {
    let t = Table::grid(0, 0);
    assert_eq!(t.col_count(), 1);
    assert_eq!(t.bodies[0].body_rows.len(), 1);
}

#[test]
fn insert_block_after_adds_a_table_that_round_trips() {
    let loro = document_to_loro(&doc_with_paragraph("hello")).unwrap();
    let new_index =
        insert_block_after(&loro, 0, &Block::Table(Box::new(Table::grid(2, 2)))).unwrap();
    assert_eq!(new_index, 1, "table lands right after the paragraph");

    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(rebuilt.sections[0].blocks.len(), 2);
    let Block::Para(p) = &rebuilt.sections[0].blocks[0] else {
        panic!("block 0 stays the paragraph");
    };
    assert_eq!(para_text(p), "hello");
    assert!(
        matches!(rebuilt.sections[0].blocks[1], Block::Table(_)),
        "block 1 is the inserted table"
    );
}

#[test]
fn inserted_table_cells_are_live_editable_containers() {
    let loro = document_to_loro(&doc_with_paragraph("x")).unwrap();
    insert_block_after(&loro, 0, &Block::Table(Box::new(Table::grid(1, 2)))).unwrap();
    // The table is block 1; its first cell starts empty and accepts text.
    assert_eq!(get_block_text_at(&loro, &BlockPath::in_cell(1, 0, 0)), "");
    insert_text_at(&loro, &BlockPath::in_cell(1, 0, 0), 0, "hi").unwrap();

    let rebuilt = loro_to_document(&loro).unwrap();
    let Block::Table(t) = &rebuilt.sections[0].blocks[1] else {
        panic!("table");
    };
    let Block::Para(cell0) = &t.bodies[0].body_rows[0].cells[0].blocks[0] else {
        panic!("cell para");
    };
    assert_eq!(para_text(cell0), "hi");
}

// ── insert_inline_note_at ───────────────────────────────────────────────────

#[test]
fn insert_footnote_at_cursor_round_trips_with_body() {
    let loro = document_to_loro(&doc_with_paragraph("abc")).unwrap();
    let body = vec![Block::Para(vec![Inline::Str("note".into())])];
    insert_inline_note_at(&loro, &BlockPath::block(0), 3, &NoteKind::Footnote, &body).unwrap();

    // The note's body is a live container, addressable and editable.
    assert_eq!(
        get_block_text_at(&loro, &BlockPath::in_note(0, 0, 0)),
        "note"
    );

    let rebuilt = loro_to_document(&loro).unwrap();
    let Block::Para(inlines) = &rebuilt.sections[0].blocks[0] else {
        panic!("para");
    };
    let note = inlines
        .iter()
        .find_map(|i| match i {
            Inline::Note(kind, body) => Some((kind, body)),
            _ => None,
        })
        .expect("note inline present");
    assert_eq!(*note.0, NoteKind::Footnote);
    let Block::Para(bp) = &note.1[0] else {
        panic!("note body para");
    };
    assert_eq!(para_text(bp), "note");
}

#[test]
fn inserted_empty_footnote_body_accepts_typing() {
    let loro = document_to_loro(&doc_with_paragraph("ref")).unwrap();
    // The editor inserts a footnote with one empty paragraph, then the user types.
    insert_inline_note_at(
        &loro,
        &BlockPath::block(0),
        3,
        &NoteKind::Footnote,
        &[Block::Para(Vec::new())],
    )
    .unwrap();
    assert_eq!(get_block_text_at(&loro, &BlockPath::in_note(0, 0, 0)), "");
    insert_text_at(&loro, &BlockPath::in_note(0, 0, 0), 0, "typed").unwrap();

    let rebuilt = loro_to_document(&loro).unwrap();
    let Block::Para(inlines) = &rebuilt.sections[0].blocks[0] else {
        panic!("para");
    };
    let body = inlines
        .iter()
        .find_map(|i| match i {
            Inline::Note(_, b) => Some(b),
            _ => None,
        })
        .expect("note");
    let Block::Para(bp) = &body[0] else {
        panic!("body para");
    };
    assert_eq!(para_text(bp), "typed");
}

#[test]
fn insert_footnote_inside_a_table_cell() {
    let loro = document_to_loro(&doc_with_paragraph("x")).unwrap();
    insert_block_after(&loro, 0, &Block::Table(Box::new(Table::grid(1, 1)))).unwrap();
    insert_text_at(&loro, &BlockPath::in_cell(1, 0, 0), 0, "c").unwrap();

    // Insert a footnote after the cell's "c"; the note descends Cell → Note.
    let cell_path = BlockPath::in_cell(1, 0, 0);
    insert_inline_note_at(
        &loro,
        &cell_path,
        1,
        &NoteKind::Footnote,
        &[Block::Para(vec![Inline::Str("fn".into())])],
    )
    .unwrap();

    let nested = BlockPath {
        root: 1,
        steps: vec![
            PathStep::Cell { cell: 0, block: 0 },
            PathStep::Note { note: 0, block: 0 },
        ],
    };
    assert_eq!(get_block_text_at(&loro, &nested), "fn");
}
