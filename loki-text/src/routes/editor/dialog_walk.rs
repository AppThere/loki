// SPDX-License-Identifier: Apache-2.0

//! One depth-first walk of a document's blocks and inlines, shared by every
//! dialog that has to inspect the whole document.
//!
//! # Why this is shared rather than written per dialog
//!
//! Four dialogs grew their own walker — the publish preflight's image and table
//! audits, the font collector, and the link dialog's anchor picker — and all
//! four independently descended into `table.bodies` while missing
//! `table.head.rows` and `table.foot.rows`. That is exactly where the Insert
//! table dialog puts a header row, so the one structure this feature set
//! creates by default was the one structure the audits could not see: an
//! undescribed image in a header cell made the preflight report "all images
//! have alternative text", and a device font used only in a header made the
//! Fonts tab report "everything is bundled".
//!
//! Container variants went the same way. `Block::StyledPara` — the variant an
//! app with a paragraph style dialog produces constantly — was handled by one
//! walker out of four, so the statistics tab could report a real word count
//! beside zero paragraphs.
//!
//! A walk that is written once is wrong at most once, and the tests below pin
//! the two structures that were actually missed.

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::{BlockPath, PathStep};

/// Calls `f` on every block in `doc`, depth first, including blocks nested in
/// list items, quotes, figures, notes, text boxes and **every** table cell.
pub(in crate::routes::editor) fn visit_doc_blocks(doc: &Document, f: &mut impl FnMut(&Block)) {
    for section in &doc.sections {
        visit_blocks(&section.blocks, f);
    }
}

/// Calls `f` on every inline reachable from `doc`, depth first.
pub(in crate::routes::editor) fn visit_doc_inlines(doc: &Document, f: &mut impl FnMut(&Inline)) {
    visit_doc_blocks(doc, &mut |block| {
        for inlines in block_inlines(block) {
            visit_inlines(inlines, f);
        }
    });
}

/// Calls `f` on every block in `blocks` and everything nested inside them.
pub(in crate::routes::editor) fn visit_blocks(blocks: &[Block], f: &mut impl FnMut(&Block)) {
    for block in blocks {
        f(block);
        match block {
            Block::BlockQuote(inner) | Block::Figure(_, _, inner) => visit_blocks(inner, f),
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                for item in items {
                    visit_blocks(item, f);
                }
            }
            Block::DefinitionList(items) => {
                for (_, defs) in items {
                    for def in defs {
                        visit_blocks(def, f);
                    }
                }
            }
            Block::Table(table) => {
                for cell in table_cell_blocks(table) {
                    visit_blocks(cell, f);
                }
            }
            _ => {}
        }
        // Blocks reachable only through an inline (notes, text boxes).
        for inlines in block_inlines(block) {
            visit_inline_blocks(inlines, f);
        }
    }
}

/// Every cell's block list in `table`, in reading order.
///
/// **Head and foot are included.** `Table::grid` puts everything in `bodies`,
/// but `build_table` promotes the header row into `table.head.rows`, and
/// imported documents populate `foot` — a walk that visits only `bodies` misses
/// both.
pub(in crate::routes::editor) fn table_cell_blocks(table: &Table) -> Vec<&Vec<Block>> {
    let body_rows = table
        .bodies
        .iter()
        .flat_map(|b| b.head_rows.iter().chain(&b.body_rows));
    table
        .head
        .rows
        .iter()
        .chain(body_rows)
        .chain(&table.foot.rows)
        .flat_map(|row| row.cells.iter().map(|cell| &cell.blocks))
        .collect()
}

/// The inline lists a block carries directly.
pub(in crate::routes::editor) fn block_inlines(block: &Block) -> Vec<&Vec<Inline>> {
    match block {
        Block::Plain(inlines) | Block::Para(inlines) | Block::Heading(_, _, inlines) => {
            vec![inlines]
        }
        // The variant an app with a paragraph style dialog produces constantly.
        Block::StyledPara(para) => vec![&para.inlines],
        Block::LineBlock(lines) => lines.iter().collect(),
        Block::DefinitionList(items) => items.iter().map(|(term, _)| term).collect(),
        Block::Table(table) => vec![&table.caption.full],
        _ => Vec::new(),
    }
}

/// Calls `f` on every inline in `inlines` and everything nested inside them.
pub(in crate::routes::editor) fn visit_inlines(inlines: &[Inline], f: &mut impl FnMut(&Inline)) {
    for inline in inlines {
        f(inline);
        for inner in inline_children(inline) {
            visit_inlines(inner, f);
        }
    }
}

/// Recurses into the blocks an inline can carry (notes and text boxes).
fn visit_inline_blocks(inlines: &[Inline], f: &mut impl FnMut(&Block)) {
    for inline in inlines {
        match inline {
            Inline::Note(_, blocks) | Inline::TextBox(_, blocks) => visit_blocks(blocks, f),
            _ => {
                for inner in inline_children(inline) {
                    visit_inline_blocks(inner, f);
                }
            }
        }
    }
}

/// The inline lists an inline carries directly.
pub(in crate::routes::editor) fn inline_children(inline: &Inline) -> Vec<&Vec<Inline>> {
    match inline {
        Inline::Emph(v)
        | Inline::Underline(v)
        | Inline::Strong(v)
        | Inline::Strikeout(v)
        | Inline::Superscript(v)
        | Inline::Subscript(v)
        | Inline::SmallCaps(v)
        | Inline::Quoted(_, v)
        | Inline::Cite(_, v)
        | Inline::Span(_, v)
        | Inline::Link(_, v, _)
        | Inline::Image(_, v, _) => vec![v],
        Inline::StyledRun(run) => vec![&run.content],
        _ => Vec::new(),
    }
}

/// The block a [`BlockPath`] addresses, if the document still has one there.
///
/// The cell order `PathStep::Cell` indexes is the bridge's flat
/// head → bodies → foot order, which is exactly the order
/// [`table_cell_blocks`] returns — so the descent is a lookup rather than a
/// second, separately-maintained traversal.
#[must_use]
pub(in crate::routes::editor) fn block_at_path<'a>(
    doc: &'a Document,
    path: &BlockPath,
) -> Option<&'a Block> {
    let mut block = doc
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .nth(path.root)?;

    for step in &path.steps {
        block = match step {
            PathStep::Cell { cell, block: idx } => {
                let Block::Table(table) = block else {
                    return None;
                };
                table_cell_blocks(table).get(*cell)?.get(*idx)?
            }
            PathStep::Note { note, block: idx } => {
                let mut bodies: Vec<&Vec<Block>> = Vec::new();
                for inlines in block_inlines(block) {
                    collect_note_bodies(inlines, &mut bodies);
                }
                bodies.get(*note)?.get(*idx)?
            }
        };
    }
    Some(block)
}

/// Collects note bodies in document order, for [`block_at_path`].
///
/// A plain recursion rather than `visit_inlines`, because the borrow has to
/// outlive the closure the visitor would hand it to.
fn collect_note_bodies<'a>(inlines: &'a [Inline], out: &mut Vec<&'a Vec<Block>>) {
    for inline in inlines {
        if let Inline::Note(_, blocks) = inline {
            out.push(blocks);
        }
        for inner in inline_children(inline) {
            collect_note_bodies(inner, out);
        }
    }
}

/// A block's plain text, in the byte space the cursor's offsets address.
#[must_use]
pub(in crate::routes::editor) fn block_text(block: &Block) -> String {
    let mut out = String::new();
    for inlines in block_inlines(block) {
        visit_inlines(inlines, &mut |inline| {
            if let Inline::Str(text) = inline {
                out.push_str(text);
            }
        });
    }
    out
}

#[cfg(test)]
#[path = "dialog_walk_tests.rs"]
mod tests;
