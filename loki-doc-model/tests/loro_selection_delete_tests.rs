// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Selection deletion ([`delete_selection_at`]): removing ranges that span one
//! or several sibling blocks, in either endpoint order, at the top level and
//! inside table cells — plus the rejection cases (cross-container endpoints,
//! non-text blocks inside the range) proving nothing is half-applied.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_doc_model::layout::section::Section;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::{BlockPath, MutationError, delete_selection_at};

fn para(s: &str) -> Block {
    Block::Para(vec![Inline::Str(s.into())])
}

/// Plain text of every top-level paragraph of section `s`.
fn para_texts(doc: &Document, s: usize) -> Vec<String> {
    doc.sections[s]
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Para(inlines) => Some(
                inlines
                    .iter()
                    .filter_map(|i| match i {
                        Inline::Str(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

fn three_para_doc() -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("Hello world"), para("middle"), para("goodbye")];
    doc
}

#[test]
fn deletes_within_a_single_block() {
    let loro = document_to_loro(&three_para_doc()).unwrap();
    // "Hello world" -> "Helld" (delete "lo wor", bytes 3..9).
    let p = BlockPath::block(0);
    let (path, byte) = delete_selection_at(&loro, (&p, 3), (&p, 9)).unwrap();
    assert_eq!((path, byte), (BlockPath::block(0), 3));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(para_texts(&rebuilt, 0), vec!["Helld", "middle", "goodbye"]);
}

#[test]
fn a_collapsed_selection_deletes_nothing() {
    let loro = document_to_loro(&three_para_doc()).unwrap();
    let p = BlockPath::block(1);
    let (path, byte) = delete_selection_at(&loro, (&p, 3), (&p, 3)).unwrap();
    assert_eq!((path, byte), (BlockPath::block(1), 3));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(
        para_texts(&rebuilt, 0),
        vec!["Hello world", "middle", "goodbye"]
    );
}

#[test]
fn collapses_a_range_spanning_three_blocks() {
    let loro = document_to_loro(&three_para_doc()).unwrap();
    // From byte 6 of "Hello world" to byte 4 of "goodbye": the tail of block
    // 0, all of "middle", and "good" vanish; the survivor keeps block 0's
    // head and block 2's tail.
    let (start, end) = (BlockPath::block(0), BlockPath::block(2));
    let (path, byte) = delete_selection_at(&loro, (&start, 6), (&end, 4)).unwrap();
    assert_eq!((path, byte), (BlockPath::block(0), 6));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(para_texts(&rebuilt, 0), vec!["Hello bye"]);
}

#[test]
fn endpoints_normalize_in_either_order() {
    let loro = document_to_loro(&three_para_doc()).unwrap();
    // Same range as above, endpoints swapped (focus before anchor).
    let (a, b) = (BlockPath::block(2), BlockPath::block(0));
    let (path, byte) = delete_selection_at(&loro, (&a, 4), (&b, 6)).unwrap();
    assert_eq!((path, byte), (BlockPath::block(0), 6));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(para_texts(&rebuilt, 0), vec!["Hello bye"]);
}

#[test]
fn reversed_offsets_within_one_block_normalize() {
    let loro = document_to_loro(&three_para_doc()).unwrap();
    let p = BlockPath::block(0);
    let (path, byte) = delete_selection_at(&loro, (&p, 9), (&p, 3)).unwrap();
    assert_eq!((path, byte), (BlockPath::block(0), 3));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(para_texts(&rebuilt, 0), vec!["Helld", "middle", "goodbye"]);
}

// ── Nested containers ───────────────────────────────────────────────────────

/// `[Para("intro"), Table]` — the table (global block 1) has one body row of
/// two cells; cell 0 holds paragraphs "alpha" | "beta", cell 1 holds "z".
fn doc_with_two_block_cell() -> Document {
    let cell = Cell::simple(vec![para("alpha"), para("beta")]);
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![
            cell,
            Cell::simple(vec![para("z")]),
        ])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("intro"), Block::Table(Box::new(table))];
    doc
}

/// Plain text of every paragraph in the `cell`-th cell of the table at global
/// block index 1.
fn cell_block_texts(doc: &Document, cell: usize) -> Vec<String> {
    let Block::Table(t) = &doc.sections[0].blocks[1] else {
        panic!("expected a table");
    };
    t.bodies[0].body_rows[0].cells[cell]
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Para(inlines) => Some(
                inlines
                    .iter()
                    .filter_map(|i| match i {
                        Inline::Str(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

#[test]
fn collapses_a_range_inside_one_table_cell() {
    let loro = document_to_loro(&doc_with_two_block_cell()).unwrap();
    // "alpha"|"beta" -> "alta": from byte 2 of "alpha" to byte 2 of "beta".
    let (a, b) = (BlockPath::in_cell(1, 0, 0), BlockPath::in_cell(1, 0, 1));
    let (path, byte) = delete_selection_at(&loro, (&a, 2), (&b, 2)).unwrap();
    assert_eq!((path, byte), (BlockPath::in_cell(1, 0, 0), 2));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(cell_block_texts(&rebuilt, 0), vec!["alta"]);
    assert_eq!(cell_block_texts(&rebuilt, 1), vec!["z"]);
}

#[test]
fn rejects_endpoints_in_different_cells_without_mutating() {
    let loro = document_to_loro(&doc_with_two_block_cell()).unwrap();
    let (a, b) = (BlockPath::in_cell(1, 0, 0), BlockPath::in_cell(1, 1, 0));
    let err = delete_selection_at(&loro, (&a, 1), (&b, 1));
    assert!(matches!(err, Err(MutationError::CrossContainerSelection)));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(cell_block_texts(&rebuilt, 0), vec!["alpha", "beta"]);
    assert_eq!(cell_block_texts(&rebuilt, 1), vec!["z"]);
}

#[test]
fn rejects_a_body_to_cell_selection_without_mutating() {
    let loro = document_to_loro(&doc_with_two_block_cell()).unwrap();
    let (a, b) = (BlockPath::block(0), BlockPath::in_cell(1, 0, 0));
    let err = delete_selection_at(&loro, (&a, 1), (&b, 1));
    assert!(matches!(err, Err(MutationError::CrossContainerSelection)));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(para_texts(&rebuilt, 0), vec!["intro"]);
    assert_eq!(cell_block_texts(&rebuilt, 0), vec!["alpha", "beta"]);
}

#[test]
fn rejects_a_range_containing_a_table_without_mutating() {
    // [Para, Table, Para]: a top-level selection from block 0 to block 2 would
    // swallow the table — the pre-validation must reject it untouched.
    let mut doc = doc_with_two_block_cell();
    doc.sections[0].blocks.push(para("outro"));
    let loro = document_to_loro(&doc).unwrap();
    let (a, b) = (BlockPath::block(0), BlockPath::block(2));
    let err = delete_selection_at(&loro, (&a, 1), (&b, 1));
    assert!(matches!(err, Err(MutationError::TextNotFound(_))));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(para_texts(&rebuilt, 0), vec!["intro", "outro"]);
    assert_eq!(cell_block_texts(&rebuilt, 0), vec!["alpha", "beta"]);
}

#[test]
fn rejects_a_cross_section_selection_without_mutating() {
    // Two sections of one paragraph each: global blocks 0 and 1 are top-level
    // siblings by index but live in different section lists.
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("first")];
    let mut s1 = Section::new();
    s1.blocks = vec![para("second")];
    doc.sections.push(s1);
    let loro = document_to_loro(&doc).unwrap();
    let (a, b) = (BlockPath::block(0), BlockPath::block(1));
    let err = delete_selection_at(&loro, (&a, 1), (&b, 1));
    assert!(matches!(err, Err(MutationError::CrossContainerSelection)));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(para_texts(&rebuilt, 0), vec!["first"]);
    assert_eq!(para_texts(&rebuilt, 1), vec!["second"]);
}
