// SPDX-License-Identifier: Apache-2.0

//! The document walks the preflight's checks are computed from.
//!
//! Pure functions over a [`Document`], with no Dioxus scope in sight, so the
//! nesting rule and the two accessibility audits are testable directly.

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;

use super::super::dialog_walk::{visit_doc_blocks, visit_doc_inlines};

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
    visit_doc_blocks(doc, &mut |block| {
        if let Block::Heading(level, _, _) = block {
            levels.push((*level).clamp(1, 6));
        }
    });
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
    visit_doc_blocks(doc, &mut |block| {
        if let Block::Table(table) = block {
            audit.total += 1;
            let has_caption = !table.caption.full.is_empty();
            let has_header =
                !table.head.rows.is_empty() || table.bodies.iter().any(|b| !b.head_rows.is_empty());
            if has_caption && has_header {
                audit.accessible += 1;
            }
        }
    });
    audit
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
    visit_doc_inlines(doc, &mut |inline| {
        if let Inline::Image(_, alt, _) = inline {
            audit.total += 1;
            // The alt text is the image's inline content — empty means the
            // reader is told nothing.
            if !alt.is_empty() {
                audit.described += 1;
            }
        }
    });
    audit
}
