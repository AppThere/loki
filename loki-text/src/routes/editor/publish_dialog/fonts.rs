// SPDX-License-Identifier: Apache-2.0

//! Which faces the document uses, and whether each can be embedded (design
//! note 29).
//!
//! # The one place bundled-vs-device has a licence consequence
//!
//! Everywhere else in Loki the distinction is about rendering. Here it decides
//! whether a face can be **put in the file**: a bundled face is licensed for
//! redistribution, a device face is licensed to the device. A reader without it
//! substitutes, and there is nothing the export can do about that — so the
//! table says so per face rather than in a footnote.

use std::collections::BTreeSet;

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_fonts::is_bundled_family;

/// One face the document asks for.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct UsedFace {
    /// The family name as the document names it.
    pub name: String,
    /// Whether it ships with Loki and can therefore be embedded.
    pub bundled: bool,
}

/// Every family the document references, sorted, deduplicated.
///
/// Collected from **style definitions and direct runs together**: a face used
/// only by a character style still ends up in the file, and a table that listed
/// direct formatting alone would under-report what the export must carry.
#[must_use]
pub(super) fn used_faces(doc: &Document) -> Vec<UsedFace> {
    let mut names: BTreeSet<String> = BTreeSet::new();

    for style in doc.styles.paragraph_styles.values() {
        if let Some(name) = style.char_props.font_name.as_ref() {
            insert_name(&mut names, name);
        }
    }
    for style in doc.styles.character_styles.values() {
        if let Some(name) = style.char_props.font_name.as_ref() {
            insert_name(&mut names, name);
        }
    }
    for section in &doc.sections {
        walk_blocks(&section.blocks, &mut names);
    }

    names
        .into_iter()
        .map(|name| UsedFace {
            bundled: is_bundled_family(&name),
            name,
        })
        .collect()
}

/// Adds a family name, ignoring blanks.
fn insert_name(names: &mut BTreeSet<String>, name: &str) {
    let trimmed = name.trim();
    if !trimmed.is_empty() {
        names.insert(trimmed.to_string());
    }
}

fn walk_blocks(blocks: &[Block], names: &mut BTreeSet<String>) {
    for block in blocks {
        match block {
            Block::Para(inlines) | Block::Plain(inlines) | Block::Heading(_, _, inlines) => {
                walk_inlines(inlines, names);
            }
            Block::StyledPara(para) => {
                if let Some(props) = para.direct_char_props.as_ref()
                    && let Some(name) = props.font_name.as_ref()
                {
                    insert_name(names, name);
                }
                walk_inlines(&para.inlines, names);
            }
            Block::BlockQuote(inner) => walk_blocks(inner, names),
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                for item in items {
                    walk_blocks(item, names);
                }
            }
            Block::Table(table) => {
                for body in &table.bodies {
                    for row in body.head_rows.iter().chain(&body.body_rows) {
                        for cell in &row.cells {
                            walk_blocks(&cell.blocks, names);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn walk_inlines(inlines: &[Inline], names: &mut BTreeSet<String>) {
    for inline in inlines {
        match inline {
            Inline::StyledRun(run) => {
                if let Some(props) = run.direct_props.as_ref()
                    && let Some(name) = props.font_name.as_ref()
                {
                    insert_name(names, name);
                }
                walk_inlines(&run.content, names);
            }
            Inline::Span(_, inner)
            | Inline::Emph(inner)
            | Inline::Strong(inner)
            | Inline::Underline(inner)
            | Inline::Link(_, inner, _) => walk_inlines(inner, names),
            Inline::Note(_, blocks) => walk_blocks(blocks, names),
            _ => {}
        }
    }
}

/// How many of the used faces can be embedded.
#[must_use]
pub(super) fn embeddable_count(faces: &[UsedFace]) -> usize {
    faces.iter().filter(|f| f.bundled).count()
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
