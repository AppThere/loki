// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Footnote/endnote bodies as **live CRDT containers** (not JSON blobs in the
//! mark): the body lives under the block's `KEY_NOTES` container, so it is
//! editable/mergeable like a table cell, and adjacent notes keep distinct
//! bodies (their `(kind, idx)` marks do not merge into one span).

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::loro_schema::{KEY_BLOCKS, KEY_NOTES, KEY_SECTIONS};

fn para(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.into())])
}

fn doc_with_block(block: Block) -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![block];
    doc
}

fn round_trip(doc: &Document) -> Document {
    loro_to_document(&document_to_loro(doc).unwrap()).unwrap()
}

/// Navigates to the first block's `KEY_NOTES` movable list and returns, for each
/// entry, whether it is itself a movable list (a live body container) and its
/// block count. `None` when there is no notes container.
fn note_bodies_shape(doc: &Document) -> Option<Vec<usize>> {
    let loro = document_to_loro(doc).unwrap();
    let sections = loro.get_list(KEY_SECTIONS);
    let sec = sections.get(0)?.into_container().ok()?.into_map().ok()?;
    let blocks = sec
        .get(KEY_BLOCKS)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()?;
    let block = blocks.get(0)?.into_container().ok()?.into_map().ok()?;
    let notes = block
        .get(KEY_NOTES)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()?;
    let mut shape = Vec::new();
    for i in 0..notes.len() {
        // Each entry must be a live movable-list container, not a JSON string.
        let body = notes
            .get(i)?
            .into_container()
            .ok()?
            .into_movable_list()
            .ok()?;
        shape.push(body.len());
    }
    Some(shape)
}

#[test]
fn note_body_is_a_live_container_not_a_blob() {
    let block = Block::Para(vec![
        Inline::Str("see".into()),
        Inline::Note(NoteKind::Footnote, vec![para("a"), para("b")]),
    ]);
    let doc = doc_with_block(block.clone());
    // One note whose body container holds two blocks.
    assert_eq!(note_bodies_shape(&doc), Some(vec![2]));
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

#[test]
fn two_adjacent_footnotes_keep_distinct_bodies() {
    // The classic failure mode: identical marks would merge the two anchors
    // into one delta span. The (kind, idx) mark keeps them distinct.
    let block = Block::Para(vec![
        Inline::Str("x".into()),
        Inline::Note(NoteKind::Footnote, vec![para("first")]),
        Inline::Note(NoteKind::Footnote, vec![para("second")]),
        Inline::Str("y".into()),
    ]);
    let doc = doc_with_block(block.clone());
    assert_eq!(note_bodies_shape(&doc), Some(vec![1, 1]));
    let recovered = round_trip(&doc);
    assert_eq!(recovered.sections[0].blocks[0], block);
    let Block::Para(inlines) = &recovered.sections[0].blocks[0] else {
        panic!("para");
    };
    assert_eq!(inlines.len(), 4, "Str, Note, Note, Str: {inlines:?}");
}

#[test]
fn mixed_footnote_and_endnote_preserve_kind_and_order() {
    let block = Block::Para(vec![
        Inline::Note(NoteKind::Endnote, vec![para("end")]),
        Inline::Str(" mid ".into()),
        Inline::Note(NoteKind::Footnote, vec![para("foot")]),
    ]);
    let doc = doc_with_block(block.clone());
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

#[test]
fn paragraph_without_notes_has_no_notes_container() {
    let doc = doc_with_block(para("plain text"));
    assert_eq!(note_bodies_shape(&doc), None);
}
