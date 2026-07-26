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
use futures_channel::oneshot;
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
            Block::Figure(_, caption, content) => {
                // The module contract counts figure captions; `caption.full`
                // holds the caption's block content.
                count_blocks(&caption.full, counter);
                count_blocks(content, counter);
            }
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

/// Localised status-bar word-count label (`editor-word-count`), recomputed off
/// the UI thread whenever the document changes.
///
/// # Why this is a task and not a memo (I-10)
///
/// It was a `use_memo` calling [`count_words`] inline, which had two defects.
///
/// **The reported one:** the label read `0` until the first interaction. The memo's
/// only dependency is the cursor's mirrored `document_generation`, and the
/// fresh-open path bumps `DocumentState::generation` without mirroring it — so the
/// memo ran once at mount, when `state.document` was still `None`, produced `0`,
/// and did not re-run until the first edit. The mirror is now written at the seed
/// publish (`editor_inner`), which is the actual fix; this function's part is to
/// stop publishing a count nobody has computed.
///
/// **The unreported one, found by measuring:** the walk is O(document) and ran
/// synchronously on the UI thread *per mutation*. Measured at 0.32 ms for 10
/// pages, 3.0 ms for 100, **15.3 ms for 500** and 62.8 ms for 2000 — so from a few
/// hundred pages on, every keystroke dropped a frame on the count alone. That was
/// pre-existing and is not what I-10 describes, but the same fix removes it.
///
/// # Why there is no size threshold
///
/// Deciding "inline if small" needs the document's size before counting it, and
/// the only honest proxy (block count) does not bound text length — one paragraph
/// can hold a megabyte. So every count goes to a worker, and the label keeps the
/// **previous** value while a new one is computed rather than flickering to an
/// indeterminate state on each keystroke. `None` — and so the pending label —
/// therefore appears only when there is no previous value, which is exactly the
/// load case I-10 is about.
pub fn use_word_count_label(
    doc_state: Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
) -> Memo<String> {
    // Narrow the change signal. Reading `cursor_state` directly would subscribe
    // to *every* CursorState write — each cursor move, click, and drag update —
    // spawning a counting worker even when the document did not change. This
    // cheap intermediate memo reads the generation on every write, but its `u64`
    // output only changes on a real mutation.
    let generation = use_memo(move || cursor_state.read().document_generation);

    // `None` = no count has landed yet. Distinct from `Some(0)`, which is a real
    // count of an empty document — the distinction I-10 turns on.
    let mut count = use_signal(|| None::<usize>);

    use_effect(move || {
        let observed_gen = generation();
        let Some(doc) = doc_state
            .lock()
            .ok()
            .and_then(|state| state.document.clone())
        else {
            // No document yet. Leave any previous value alone rather than
            // publishing a `0` for a document that has not arrived.
            return;
        };
        spawn(async move {
            let (tx, rx) = oneshot::channel();
            let worker = std::thread::Builder::new()
                .name("loki-word-count".into())
                // `doc` is an `Arc<Document>`, so this moves a refcount rather
                // than the document.
                .spawn(move || {
                    let _ = tx.send(count_words(&doc));
                });
            if worker.is_err() {
                return;
            }
            let Ok(counted) = rx.await else { return };
            // Discard a superseded worker. Workers are not ordered, so a slow
            // count of an older document could otherwise land after a fast count
            // of a newer one and leave the label describing a document that no
            // longer exists.
            if *generation.peek() == observed_gen {
                count.set(Some(counted));
            }
        });
    });

    use_memo(move || match count() {
        Some(words) => fl!("editor-word-count", count = words as i64),
        None => fl!("editor-word-count-pending"),
    })
}
