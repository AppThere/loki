// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, NoteKind};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;

use super::count_words;

fn doc_with(blocks: Vec<Block>) -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks = blocks;
    doc
}

#[test]
fn counts_whitespace_separated_words() {
    let doc = doc_with(vec![Block::Para(vec![Inline::Str(
        "the quick  brown fox".into(),
    )])]);
    assert_eq!(count_words(&doc), 4);
}

#[test]
fn empty_document_counts_zero() {
    assert_eq!(count_words(&doc_with(vec![])), 0);
    assert_eq!(
        count_words(&doc_with(vec![Block::Para(vec![Inline::Str(
            "   ".into()
        )])])),
        0
    );
}

#[test]
fn styling_inside_a_word_does_not_split_it() {
    // "Hel" + bold "lo" + " world" = 2 words, not 3.
    let doc = doc_with(vec![Block::Para(vec![
        Inline::Str("Hel".into()),
        Inline::Strong(vec![Inline::Str("lo".into())]),
        Inline::Str(" world".into()),
    ])]);
    assert_eq!(count_words(&doc), 2);
}

#[test]
fn explicit_space_and_break_inlines_separate_words() {
    let doc = doc_with(vec![Block::Para(vec![
        Inline::Str("one".into()),
        Inline::Space,
        Inline::Str("two".into()),
        Inline::LineBreak,
        Inline::Str("three".into()),
    ])]);
    assert_eq!(count_words(&doc), 3);
}

#[test]
fn block_boundaries_separate_words() {
    // "…end" | "start…" must not merge into one word across paragraphs.
    let doc = doc_with(vec![
        Block::Para(vec![Inline::Str("end".into())]),
        Block::Para(vec![Inline::Str("start".into())]),
    ]);
    assert_eq!(count_words(&doc), 2);
}

#[test]
fn table_cells_are_counted() {
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![
            Cell::simple(vec![Block::Para(vec![Inline::Str("alpha beta".into())])]),
            Cell::simple(vec![Block::Para(vec![Inline::Str("gamma".into())])]),
        ])])],
        foot: TableFoot::empty(),
    };
    let doc = doc_with(vec![
        Block::Para(vec![Inline::Str("intro".into())]),
        Block::Table(Box::new(table)),
    ]);
    assert_eq!(count_words(&doc), 4);
}

#[test]
fn footnote_bodies_are_excluded() {
    // Matches Word's status-bar semantics (footnotes not included).
    let doc = doc_with(vec![Block::Para(vec![
        Inline::Str("body".into()),
        Inline::Note(
            NoteKind::Footnote,
            vec![Block::Para(vec![Inline::Str("hidden note words".into())])],
        ),
        Inline::Str("more".into()),
    ])]);
    assert_eq!(count_words(&doc), 2);
}

#[test]
fn lists_and_headings_are_counted() {
    let doc = doc_with(vec![
        Block::Heading(1, NodeAttr::default(), vec![Inline::Str("Title".into())]),
        Block::BulletList(vec![
            vec![Block::Para(vec![Inline::Str("first item".into())])],
            vec![Block::Para(vec![Inline::Str("second".into())])],
        ]),
    ]);
    assert_eq!(count_words(&doc), 4);
}
