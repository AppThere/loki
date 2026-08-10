// SPDX-License-Identifier: Apache-2.0

//! The document walks the preflight's checks are computed from.
//!
//! Pure functions over a [`Document`], with no Dioxus scope in sight, so the
//! nesting rule and the two accessibility audits are testable directly.

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;

/// What the document's headings look like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct HeadingStructure {
    /// How many headings there are.
    pub headings: usize,
    /// Whether the outline descends without skipping a level.
    pub well_nested: bool,
}

/// Audits the heading outline.
///
/// "Well nested" means no level is skipped on the way *down* — an `h1` followed
/// by an `h3` leaves a reader's navigation with a hole. Coming back *up* any
/// distance is fine, which is why only descents are checked.
#[must_use]
pub(super) fn heading_structure(doc: &Document) -> HeadingStructure {
    let mut levels = Vec::new();
    for section in &doc.sections {
        collect_levels(&section.blocks, &mut levels);
    }
    let mut well_nested = true;
    let mut previous: Option<u8> = None;
    for level in &levels {
        if let Some(prev) = previous
            && *level > prev + 1
        {
            well_nested = false;
        }
        previous = Some(*level);
    }
    HeadingStructure {
        headings: levels.len(),
        well_nested,
    }
}

/// Collects heading levels in document order.
fn collect_levels(blocks: &[Block], out: &mut Vec<u8>) {
    for block in blocks {
        match block {
            Block::Heading(level, _, _) => out.push((*level).clamp(1, 6)),
            Block::BlockQuote(inner) => collect_levels(inner, out),
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                for item in items {
                    collect_levels(item, out);
                }
            }
            _ => {}
        }
    }
}

/// How many tables carry what a reader needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct TableAudit {
    /// Tables in the document.
    pub total: usize,
    /// Tables with both a caption and a header row.
    pub accessible: usize,
}

/// Audits the document's tables.
#[must_use]
pub(super) fn table_audit(doc: &Document) -> TableAudit {
    let mut audit = TableAudit::default();
    for section in &doc.sections {
        walk_tables(&section.blocks, &mut audit);
    }
    audit
}

fn walk_tables(blocks: &[Block], audit: &mut TableAudit) {
    for block in blocks {
        match block {
            Block::Table(table) => {
                audit.total += 1;
                let has_caption = !table.caption.full.is_empty();
                let has_header = !table.head.rows.is_empty()
                    || table.bodies.iter().any(|b| !b.head_rows.is_empty());
                if has_caption && has_header {
                    audit.accessible += 1;
                }
                for body in &table.bodies {
                    for row in body.head_rows.iter().chain(&body.body_rows) {
                        for cell in &row.cells {
                            walk_tables(&cell.blocks, audit);
                        }
                    }
                }
            }
            Block::BlockQuote(inner) => walk_tables(inner, audit),
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                for item in items {
                    walk_tables(item, audit);
                }
            }
            _ => {}
        }
    }
}

/// How many images carry alternative text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ImageAudit {
    /// Images in the document.
    pub total: usize,
    /// Images with non-empty alternative text.
    pub described: usize,
}

/// Audits the document's images.
#[must_use]
pub(super) fn image_audit(doc: &Document) -> ImageAudit {
    let mut audit = ImageAudit::default();
    for section in &doc.sections {
        walk_images(&section.blocks, &mut audit);
    }
    audit
}

fn walk_images(blocks: &[Block], audit: &mut ImageAudit) {
    for block in blocks {
        match block {
            Block::Para(inlines) | Block::Plain(inlines) | Block::Heading(_, _, inlines) => {
                walk_image_inlines(inlines, audit);
            }
            Block::BlockQuote(inner) => walk_images(inner, audit),
            Block::OrderedList(_, items) | Block::BulletList(items) => {
                for item in items {
                    walk_images(item, audit);
                }
            }
            Block::Table(table) => {
                for body in &table.bodies {
                    for row in body.head_rows.iter().chain(&body.body_rows) {
                        for cell in &row.cells {
                            walk_images(&cell.blocks, audit);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn walk_image_inlines(inlines: &[Inline], audit: &mut ImageAudit) {
    for inline in inlines {
        match inline {
            Inline::Image(_, alt, _) => {
                audit.total += 1;
                // The alt text is the image's inline content — empty means the
                // reader is told nothing.
                if !alt.is_empty() {
                    audit.described += 1;
                }
            }
            Inline::StyledRun(run) => walk_image_inlines(&run.content, audit),
            Inline::Span(_, inner)
            | Inline::Emph(inner)
            | Inline::Strong(inner)
            | Inline::Link(_, inner, _) => walk_image_inlines(inner, audit),
            Inline::Note(_, blocks) => walk_images(blocks, audit),
            _ => {}
        }
    }
}
