// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Nested-container accept/reject-all ([`accept_reject_all_revisions`], Review
//! tab 4a.2): the sweep must reach tracked changes inside table cells and
//! footnote bodies, not just top-level paragraphs.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, NoteKind, StyledRun};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};
use loki_doc_model::{accept_reject_all_revisions, loro_bridge};

/// A run carrying a tracked-insertion mark by `author`.
fn insertion(author: &str, text: &str) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            revision: Some(RevisionMark::new(RevisionKind::Insertion).with_author(author)),
            ..CharProps::default()
        })),
        content: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

/// A paragraph "keep" + tracked-insertion "ins" + "tail".
fn para_with_change() -> Block {
    Block::Para(vec![
        Inline::Str("keep".into()),
        insertion("Ada", "ins"),
        Inline::Str("tail".into()),
    ])
}

/// Flattens a block's paragraph text, descending through styled runs and notes.
fn block_text(block: &Block) -> String {
    fn inl(i: &Inline) -> String {
        match i {
            Inline::Str(s) => s.clone(),
            Inline::StyledRun(r) => r.content.iter().map(inl).collect(),
            Inline::Note(_, blocks) => blocks.iter().map(block_text).collect(),
            _ => String::new(),
        }
    }
    match block {
        Block::Para(i) | Block::Plain(i) => i.iter().map(inl).collect(),
        Block::StyledPara(p) => p.inlines.iter().map(inl).collect(),
        _ => String::new(),
    }
}

/// A single-cell table whose only paragraph carries a tracked change.
fn doc_with_table_change() -> Document {
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![Cell::simple(
            vec![para_with_change()],
        )])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];
    doc
}

/// Text of the table's first cell's first paragraph after a rebuild.
fn cell_text(doc: &Document) -> String {
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("expected a table");
    };
    block_text(&t.bodies[0].body_rows[0].cells[0].blocks[0])
}

#[test]
fn accept_all_resolves_a_change_in_a_table_cell() {
    let doc = doc_with_table_change();
    assert!(doc.has_tracked_changes());
    let loro = document_to_loro(&doc).unwrap();

    let resolved = accept_reject_all_revisions(&loro, true).unwrap();
    assert_eq!(resolved, 1); // the nested cell change, previously skipped

    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(cell_text(&rebuilt), "keepinstail"); // accepted insertion kept
    assert!(!rebuilt.has_tracked_changes()); // and the document is clean
}

#[test]
fn reject_all_removes_a_change_in_a_table_cell() {
    let loro = document_to_loro(&doc_with_table_change()).unwrap();

    let resolved = accept_reject_all_revisions(&loro, false).unwrap();
    assert_eq!(resolved, 1);

    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(cell_text(&rebuilt), "keeptail"); // rejected insertion removed
    assert!(!rebuilt.has_tracked_changes());
}

/// A paragraph whose footnote body carries a tracked change.
fn doc_with_footnote_change() -> Document {
    let note = Inline::Note(NoteKind::Footnote, vec![para_with_change()]);
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str("body".into()), note])];
    doc
}

/// Text of the footnote body's first paragraph after a rebuild.
fn note_body_text(doc: &Document) -> String {
    let inlines = match &doc.sections[0].blocks[0] {
        Block::Para(i) => i,
        _ => panic!("expected a paragraph"),
    };
    inlines
        .iter()
        .find_map(|i| match i {
            Inline::Note(_, blocks) => Some(block_text(&blocks[0])),
            _ => None,
        })
        .expect("footnote survives")
}

#[test]
fn accept_all_resolves_a_change_in_a_footnote_body() {
    let doc = doc_with_footnote_change();
    assert!(doc.has_tracked_changes());
    // Guard: the footnote body genuinely round-trips through the bridge.
    let round = loro_bridge::loro_to_document(&document_to_loro(&doc).unwrap()).unwrap();
    assert_eq!(note_body_text(&round), "keepinstail");

    let loro = document_to_loro(&doc).unwrap();
    let resolved = accept_reject_all_revisions(&loro, true).unwrap();
    assert_eq!(resolved, 1);

    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(note_body_text(&rebuilt), "keepinstail");
    assert!(!rebuilt.has_tracked_changes());
}
