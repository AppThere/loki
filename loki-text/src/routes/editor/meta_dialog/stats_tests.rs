// SPDX-License-Identifier: Apache-2.0

//! Tests for the derived document statistics.

use super::*;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{LinkTarget, NoteKind};
use loki_doc_model::content::table::core::Table;
use loki_doc_model::layout::section::Section;

fn words(s: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    for (i, word) in s.split(' ').enumerate() {
        if i > 0 {
            out.push(Inline::Space);
        }
        out.push(Inline::Str(word.to_string()));
    }
    out
}

fn doc(blocks: Vec<Block>) -> Document {
    let mut d = Document::default();
    d.sections = vec![Section {
        blocks,
        ..Default::default()
    }];
    d
}

#[test]
fn an_empty_document_counts_nothing() {
    assert_eq!(collect(&doc(Vec::new())), DocStats::default());
}

/// Headings are body text: a document of ten chapter titles has ten
/// paragraphs, not zero.
#[test]
fn paragraphs_and_headings_both_count_as_paragraphs() {
    let stats = collect(&doc(vec![
        Block::Heading(1, NodeAttr::default(), words("One")),
        Block::Para(words("body text here")),
    ]));
    assert_eq!(stats.paragraphs, 2);
    assert_eq!(stats.words, 4);
}

/// Characters include the spaces between words — the count a publisher's
/// contract means by "characters".
#[test]
fn characters_include_spaces() {
    let stats = collect(&doc(vec![Block::Para(words("ab cd"))]));
    assert_eq!(stats.characters, 5);
}

/// Multi-byte text is counted in characters, not bytes: an em dash is one
/// character, and a byte count would report three.
#[test]
fn multibyte_text_counts_characters_not_bytes() {
    let stats = collect(&doc(vec![Block::Para(vec![Inline::Str(
        "l\u{2019}heure\u{2014}bleue".to_string(),
    )])]));
    assert_eq!(
        stats.characters,
        "l\u{2019}heure\u{2014}bleue".chars().count()
    );
}

/// Content nested in quotes, lists and table cells is still the document's
/// content — a count that stopped at the first container would understate
/// every structured document.
#[test]
fn nested_content_is_counted() {
    let mut table = Table::grid(2, 2);
    if let Some(body) = table.bodies.first_mut()
        && let Some(row) = body.body_rows.first_mut()
        && let Some(cell) = row.cells.first_mut()
    {
        cell.blocks = vec![Block::Para(words("in a cell"))];
    }

    let stats = collect(&doc(vec![
        Block::BlockQuote(vec![Block::Para(words("quoted line"))]),
        Block::BulletList(vec![vec![Block::Para(words("list item"))]]),
        Block::Table(Box::new(table)),
    ]));
    assert_eq!(stats.tables, 1);
    assert!(stats.words >= 7, "quote, list and cell text all counted");
    assert!(stats.paragraphs >= 3);
}

#[test]
fn images_and_notes_are_counted() {
    let stats = collect(&doc(vec![Block::Para(vec![
        Inline::Str("text".to_string()),
        Inline::Image(
            NodeAttr::default(),
            Vec::new(),
            LinkTarget::new("cover.png"),
        ),
        Inline::Note(NoteKind::Footnote, vec![Block::Para(words("note body"))]),
    ])]));
    assert_eq!(stats.images, 1);
    assert_eq!(stats.notes, 1);
}

/// A table inside a footnote is still a table in the document — the
/// accessibility tab's "0 images, 0 media" claim depends on this being true.
#[test]
fn tables_and_images_inside_notes_are_still_counted() {
    let stats = collect(&doc(vec![Block::Para(vec![Inline::Note(
        NoteKind::Footnote,
        vec![
            Block::Table(Box::new(Table::grid(1, 1))),
            Block::Para(vec![Inline::Image(
                NodeAttr::default(),
                Vec::new(),
                LinkTarget::new("cover.png"),
            )]),
        ],
    )])]));
    assert_eq!(stats.notes, 1);
    assert_eq!(stats.tables, 1);
    assert_eq!(stats.images, 1);
}

/// The word count comes from the shared counter, so the dialog and the status
/// bar cannot disagree.
#[test]
fn the_word_count_matches_the_shared_counter() {
    let d = doc(vec![
        Block::Heading(1, NodeAttr::default(), words("One The harbour")),
        Block::Para(words("She set the cup down")),
    ]);
    assert_eq!(
        collect(&d).words,
        crate::editing::word_count::count_words(&d)
    );
}

/// Text inside styled runs and links is body text; a walker that stopped at
/// the run boundary would undercount every formatted document.
#[test]
fn styled_and_linked_text_is_counted() {
    let stats = collect(&doc(vec![Block::Para(vec![
        Inline::Emph(words("emphasised words")),
        Inline::Space,
        Inline::Link(
            NodeAttr::default(),
            words("link text"),
            LinkTarget::new("cover.png"),
        ),
    ])]));
    assert_eq!(stats.words, 4);
}
