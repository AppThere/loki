// SPDX-License-Identifier: Apache-2.0

//! Document statistics for the metadata dialog's read-only tab.
//!
//! # Derived, never stored
//!
//! Every count here is computed from the live document. Storing them in
//! `DocumentMeta` would give the file two answers to "how many words" — the
//! stored one and the true one — and they diverge on the first keystroke after
//! a load. The word count reuses [`crate::editing::word_count::count_words`],
//! so the dialog and the status bar cannot disagree.

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;

use crate::editing::word_count::count_words;

/// The counts the Statistics tab shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DocStats {
    /// Words, by the same rule the status bar uses.
    pub words: usize,
    /// Characters of display text, including spaces.
    pub characters: usize,
    /// Paragraphs and headings — the blocks that carry body text.
    pub paragraphs: usize,
    /// Tables at any nesting depth.
    pub tables: usize,
    /// Inline images.
    pub images: usize,
    /// Footnotes and endnotes.
    pub notes: usize,
}

/// Counts everything the Statistics tab reports, in one walk of `doc`.
///
/// One traversal rather than six: the counts are always shown together, and six
/// walks of a long document to fill one panel is six chances for them to
/// disagree about what a paragraph is.
#[must_use]
pub(super) fn collect(doc: &Document) -> DocStats {
    let mut stats = DocStats {
        words: count_words(doc),
        ..DocStats::default()
    };
    for section in &doc.sections {
        walk_blocks(&section.blocks, &mut stats);
    }
    stats
}

/// Accumulates a block list into `stats`.
fn walk_blocks(blocks: &[Block], stats: &mut DocStats) {
    for block in blocks {
        match block {
            Block::Para(inlines) | Block::Plain(inlines) | Block::Heading(_, _, inlines) => {
                stats.paragraphs += 1;
                walk_inlines(inlines, stats);
            }
            // A styled paragraph is still a paragraph. Omitting it reported a
            // real word count beside zero paragraphs and zero characters for
            // any document whose body carries a style — which, in an app with a
            // paragraph style dialog, is most of them.
            Block::StyledPara(para) => {
                stats.paragraphs += 1;
                walk_inlines(&para.inlines, stats);
            }
            Block::LineBlock(lines) => {
                stats.paragraphs += 1;
                for line in lines {
                    walk_inlines(line, stats);
                }
            }
            Block::CodeBlock(_, text) => {
                stats.paragraphs += 1;
                stats.characters += text.chars().count();
            }
            Block::BlockQuote(inner) | Block::Figure(_, _, inner) => walk_blocks(inner, stats),
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                for item in items {
                    walk_blocks(item, stats);
                }
            }
            Block::DefinitionList(items) => {
                for (term, definitions) in items {
                    walk_inlines(term, stats);
                    for definition in definitions {
                        walk_blocks(definition, stats);
                    }
                }
            }
            Block::Table(table) => {
                stats.tables += 1;
                walk_inlines(&table.caption.full, stats);
                for body in &table.bodies {
                    for row in body.head_rows.iter().chain(&body.body_rows) {
                        for cell in &row.cells {
                            walk_blocks(&cell.blocks, stats);
                        }
                    }
                }
                for row in table.head.rows.iter().chain(&table.foot.rows) {
                    for cell in &row.cells {
                        walk_blocks(&cell.blocks, stats);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Accumulates an inline sequence into `stats`.
fn walk_inlines(inlines: &[Inline], stats: &mut DocStats) {
    for inline in inlines {
        match inline {
            Inline::Str(text) => stats.characters += text.chars().count(),
            Inline::Space | Inline::SoftBreak | Inline::LineBreak => stats.characters += 1,
            Inline::Code(_, text) => stats.characters += text.chars().count(),
            Inline::Image(..) => stats.images += 1,
            // Note bodies are counted for images and tables but **not** for
            // words: `count_words` excludes them, and a character count that
            // included them would contradict the word count beside it.
            Inline::Note(_, blocks) => {
                stats.notes += 1;
                let mut inner = DocStats::default();
                walk_blocks(blocks, &mut inner);
                stats.tables += inner.tables;
                stats.images += inner.images;
            }
            Inline::StyledRun(run) => walk_inlines(&run.content, stats),
            Inline::Span(_, inner)
            | Inline::Emph(inner)
            | Inline::Strong(inner)
            | Inline::Underline(inner)
            | Inline::Strikeout(inner)
            | Inline::Superscript(inner)
            | Inline::Subscript(inner)
            | Inline::SmallCaps(inner)
            | Inline::Quoted(_, inner) => walk_inlines(inner, stats),
            Inline::Link(_, inner, _) => walk_inlines(inner, stats),
            Inline::TextBox(_, blocks) => walk_blocks(blocks, stats),
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
