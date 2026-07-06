// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::PathStep;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::loro_mutation::get_block_text;

use super::{delete_selection_in_doc, selection_start};
use crate::editing::cursor::{CursorState, DocumentPosition};

fn para(s: &str) -> Block {
    Block::Para(vec![Inline::Str(s.into())])
}

fn loro_with_paras(texts: &[&str]) -> loro::LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = texts.iter().map(|t| para(t)).collect();
    document_to_loro(&doc).unwrap()
}

fn selection(anchor: DocumentPosition, focus: DocumentPosition) -> CursorState {
    let mut cs = CursorState::new();
    cs.anchor = Some(anchor);
    cs.focus = Some(focus);
    cs
}

#[test]
fn selection_start_orders_top_level_positions() {
    let a = DocumentPosition::top_level(0, 2, 1);
    let b = DocumentPosition::top_level(1, 0, 9);
    // Block 0 precedes block 2 regardless of byte offsets or endpoint order.
    assert_eq!(selection_start(&a, &b), b);
    assert_eq!(selection_start(&b, &a), b);
}

#[test]
fn selection_start_orders_by_byte_within_one_block() {
    let a = DocumentPosition::top_level(0, 1, 7);
    let b = DocumentPosition::top_level(0, 1, 3);
    assert_eq!(selection_start(&a, &b), b);
}

#[test]
fn selection_start_orders_nested_positions_by_leaf_block() {
    let mk = |block: usize, byte: usize| DocumentPosition {
        page_index: 0,
        paragraph_index: 1,
        byte_offset: byte,
        path: vec![PathStep::Cell { cell: 0, block }],
    };
    assert_eq!(selection_start(&mk(1, 0), &mk(0, 5)), mk(0, 5));
}

#[test]
fn no_selection_deletes_nothing() {
    let loro = loro_with_paras(&["hello"]);
    let mut cs = CursorState::new();
    cs.anchor = Some(DocumentPosition::top_level(0, 0, 2));
    cs.focus = cs.anchor.clone(); // point cursor, not a range
    assert_eq!(delete_selection_in_doc(&loro, &cs), None);
    assert_eq!(get_block_text(&loro, 0), "hello");
}

#[test]
fn same_block_selection_collapses_to_range_start() {
    let loro = loro_with_paras(&["hello world"]);
    // Focus before anchor (backwards drag): bytes 3..9 selected.
    let cs = selection(
        DocumentPosition::top_level(0, 0, 9),
        DocumentPosition::top_level(0, 0, 3),
    );
    let pos = delete_selection_in_doc(&loro, &cs).unwrap();
    assert_eq!(pos, DocumentPosition::top_level(0, 0, 3));
    assert_eq!(get_block_text(&loro, 0), "helld");
}

#[test]
fn cross_block_selection_keeps_the_start_pages_index() {
    let loro = loro_with_paras(&["first", "second", "third"]);
    // Anchor on (page 3, block 2), focus on (page 1, block 0): the collapsed
    // cursor is the ordered start — focus's page.
    let cs = selection(
        DocumentPosition::top_level(3, 2, 3),
        DocumentPosition::top_level(1, 0, 2),
    );
    let pos = delete_selection_in_doc(&loro, &cs).unwrap();
    assert_eq!(pos, DocumentPosition::top_level(1, 0, 2));
    assert_eq!(get_block_text(&loro, 0), "fird");
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(rebuilt.sections[0].blocks.len(), 1);
}

#[test]
fn rejected_cross_container_selection_mutates_nothing() {
    let loro = loro_with_paras(&["top level"]);
    // Anchor at top level, focus inside a (nonexistent, but structurally
    // nested) cell path — different containers, rejected before resolution.
    let cs = selection(
        DocumentPosition::top_level(0, 0, 1),
        DocumentPosition {
            page_index: 0,
            paragraph_index: 0,
            byte_offset: 1,
            path: vec![PathStep::Cell { cell: 0, block: 0 }],
        },
    );
    assert_eq!(delete_selection_in_doc(&loro, &cs), None);
    assert_eq!(get_block_text(&loro, 0), "top level");
}
