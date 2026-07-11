// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Footnote/endnote detection for the incremental relayout gate.
//!
//! Footnotes render at section end, so any note in the changed region can
//! renumber or repaginate the tail — incremental reuse is disabled whenever a
//! note is present. Split out of `incremental.rs` to hold its line ceiling.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::document::Document;

/// `true` if any inline in `inlines` (recursively) is a footnote/endnote.
fn inlines_have_note(inlines: &[Inline]) -> bool {
    inlines.iter().any(|i| match i {
        Inline::Note(..) => true,
        Inline::Strong(c)
        | Inline::Emph(c)
        | Inline::Underline(c)
        | Inline::Strikeout(c)
        | Inline::Superscript(c)
        | Inline::Subscript(c)
        | Inline::SmallCaps(c)
        | Inline::Quoted(_, c)
        | Inline::Span(_, c)
        | Inline::Cite(_, c) => inlines_have_note(c),
        Inline::Link(_, c, _) => inlines_have_note(c),
        Inline::StyledRun(run) => inlines_have_note(&run.content),
        _ => false,
    })
}

fn table_has_note(t: &Table) -> bool {
    t.head
        .rows
        .iter()
        .chain(t.foot.rows.iter())
        .chain(
            t.bodies
                .iter()
                .flat_map(|b| b.head_rows.iter().chain(b.body_rows.iter())),
        )
        .any(|row| {
            row.cells
                .iter()
                .any(|c| c.blocks.iter().any(block_has_note))
        })
}

/// `true` if `block` (recursively) contains a footnote/endnote.
pub(crate) fn block_has_note(block: &Block) -> bool {
    match block {
        Block::Para(i) | Block::Plain(i) | Block::Heading(_, _, i) => inlines_have_note(i),
        Block::StyledPara(p) => inlines_have_note(&p.inlines),
        Block::LineBlock(lines) => lines.iter().any(|l| inlines_have_note(l)),
        Block::BlockQuote(ch) | Block::Div(_, ch) | Block::Figure(_, _, ch) => {
            ch.iter().any(block_has_note)
        }
        Block::OrderedList(_, items) | Block::BulletList(items) => {
            items.iter().flatten().any(block_has_note)
        }
        Block::Table(t) => table_has_note(t),
        _ => false,
    }
}

/// `true` if any block in `doc` contains a footnote/endnote. Computed once on
/// the full-layout path (where the cost is already O(document)).
pub fn document_has_notes(doc: &Document) -> bool {
    doc.sections
        .iter()
        .any(|s| s.blocks.iter().any(block_has_note))
}
