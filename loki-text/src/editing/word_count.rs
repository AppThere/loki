// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Live word count for the status bar (audit F7c / plan 4c.5).
//!
//! [`count_words`] streams over the document's display text without
//! allocating: a word is a maximal run of non-whitespace characters, and
//! adjacent inline runs continue the same word (`"Hel"` + bold `"lo"` is one
//! word), while block boundaries, spaces, and line breaks end it. Matching
//! Word's status-bar semantics, table cells and figure captions are counted;
//! footnote/endnote bodies, comments, and generated content (TOC, index) are
//! not.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_i18n::fl;

use super::cursor::CursorState;
use super::state::DocumentState;

#[cfg(test)]
#[path = "word_count_tests.rs"]
mod tests;

/// Streaming word-count state: `in_word` carries across adjacent text runs so
/// styling boundaries inside a word don't split it.
#[derive(Default)]
struct Counter {
    words: usize,
    in_word: bool,
}

impl Counter {
    fn text(&mut self, s: &str) {
        for c in s.chars() {
            if c.is_whitespace() {
                self.in_word = false;
            } else if !self.in_word {
                self.words += 1;
                self.in_word = true;
            }
        }
    }

    fn separator(&mut self) {
        self.in_word = false;
    }
}

/// Counts the words in `doc`'s display text. See the module docs for what is
/// and is not counted.
#[must_use]
pub fn count_words(doc: &Document) -> usize {
    let mut counter = Counter::default();
    for section in &doc.sections {
        count_blocks(&section.blocks, &mut counter);
    }
    counter.words
}

fn count_blocks(blocks: &[Block], counter: &mut Counter) {
    for block in blocks {
        counter.separator();
        match block {
            Block::Plain(inlines) | Block::Para(inlines) | Block::Heading(_, _, inlines) => {
                count_inlines(inlines, counter);
            }
            Block::StyledPara(p) => count_inlines(&p.inlines, counter),
            Block::LineBlock(lines) => {
                for line in lines {
                    counter.separator();
                    count_inlines(line, counter);
                }
            }
            Block::CodeBlock(_, code) => counter.text(code),
            Block::BlockQuote(inner) | Block::Div(_, inner) => count_blocks(inner, counter),
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                for item in items {
                    count_blocks(item, counter);
                }
            }
            Block::DefinitionList(defs) => {
                for (term, definitions) in defs {
                    counter.separator();
                    count_inlines(term, counter);
                    for def in definitions {
                        count_blocks(def, counter);
                    }
                }
            }
            Block::Table(table) => {
                for row in table
                    .head
                    .rows
                    .iter()
                    .chain(table.bodies.iter().flat_map(|b| b.body_rows.iter()))
                    .chain(table.foot.rows.iter())
                {
                    for cell in &row.cells {
                        count_blocks(&cell.blocks, counter);
                    }
                }
            }
            Block::Figure(_, _, content) => count_blocks(content, counter),
            // Generated or non-text content (and, `#[non_exhaustive]`, any
            // future block kind until it is classified) contributes nothing.
            _ => {}
        }
    }
}

fn count_inlines(inlines: &[Inline], counter: &mut Counter) {
    for inline in inlines {
        match inline {
            Inline::Str(s) => counter.text(s),
            Inline::Emph(inner)
            | Inline::Underline(inner)
            | Inline::Strong(inner)
            | Inline::Strikeout(inner)
            | Inline::Superscript(inner)
            | Inline::Subscript(inner)
            | Inline::SmallCaps(inner)
            | Inline::Quoted(_, inner)
            | Inline::Cite(_, inner)
            | Inline::Span(_, inner)
            | Inline::Link(_, inner, _) => count_inlines(inner, counter),
            Inline::StyledRun(run) => count_inlines(&run.content, counter),
            Inline::Code(_, code) => counter.text(code),
            Inline::Space | Inline::SoftBreak | Inline::LineBreak => counter.separator(),
            // Word's status-bar count excludes footnote/endnote bodies; image
            // alt text, raw passthrough, math, fields, comments, and bookmark
            // markers are not display words either. (`#[non_exhaustive]`
            // future inline kinds land here too, as separators.)
            _ => counter.separator(),
        }
    }
}

/// Memoised, localised status-bar word-count label (`editor-word-count`),
/// recomputed after every document mutation (the cursor's mirrored
/// `document_generation` is the change signal).
pub fn use_word_count_label(
    doc_state: Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
) -> Memo<String> {
    use_memo(move || {
        let _generation = cursor_state.read().document_generation;
        let count = doc_state
            .lock()
            .ok()
            .and_then(|s| s.document.as_ref().map(|d| count_words(d)))
            .unwrap_or(0);
        fl!("editor-word-count", count = count as i64)
    })
}
