// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Accept / reject of tracked changes (Review tab, 4a.2).
//!
//! Pure, format-neutral transforms over the block tree. A tracked run is an
//! [`Inline::StyledRun`] whose [`CharProps::revision`] is set (a
//! [`RevisionMark`][crate::style::props::RevisionMark]); accepting or rejecting
//! resolves every such run:
//!
//! | run kind    | accept          | reject          |
//! |-------------|-----------------|-----------------|
//! | Insertion   | keep (clear mark) | **remove** run |
//! | Deletion    | **remove** run  | keep (clear mark) |
//!
//! Whole-block tracked deletion (a deleted paragraph mark) is not modelled yet —
//! only runs are resolved. The transforms recurse through every container
//! (lists, tables, notes, block quotes, TOC/index snapshots).

use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::content::table::core::Table;
use crate::content::table::row::Row;
use crate::document::Document;
use crate::style::props::revision::RevisionKind;

/// Which direction a resolution takes.
#[derive(Clone, Copy)]
enum Resolution {
    Accept,
    Reject,
}

/// What a Backspace/Delete over one grapheme should do (Review tab, 4a.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteAction {
    /// Remove the text outright — track changes is off, or the grapheme is the
    /// author's own tracked insertion (un-typing it).
    HardDelete,
    /// Leave the text and mark it a tracked deletion (struck through).
    MarkDeleted,
    /// Do nothing — the grapheme is already a tracked deletion; the caret just
    /// steps over it.
    Skip,
}

/// Decides how deleting a grapheme carrying `existing` revision resolves when
/// `tracking` is on/off. Off ⇒ always a hard delete; on ⇒ hard-delete the
/// author's own insertion, skip an already-struck deletion, else mark it struck.
#[must_use]
pub fn delete_action(existing: Option<RevisionKind>, tracking: bool) -> DeleteAction {
    if !tracking {
        return DeleteAction::HardDelete;
    }
    match existing {
        Some(RevisionKind::Insertion) => DeleteAction::HardDelete,
        Some(RevisionKind::Deletion) => DeleteAction::Skip,
        None => DeleteAction::MarkDeleted,
    }
}

/// Whether a tracked run of `kind` is **removed** under `r` (rather than kept
/// with its mark cleared): deletions vanish on accept, insertions on reject.
fn drops(kind: RevisionKind, r: Resolution) -> bool {
    matches!(
        (r, kind),
        (Resolution::Accept, RevisionKind::Deletion)
            | (Resolution::Reject, RevisionKind::Insertion)
    )
}

/// Resolves one inline in place, returning `false` when the whole run should be
/// dropped (a rejected insertion / an accepted deletion).
fn resolve_inline(inline: &mut Inline, r: Resolution) -> bool {
    match inline {
        Inline::StyledRun(run) => {
            let drop = run
                .direct_props
                .as_ref()
                .and_then(|p| p.revision.as_ref())
                .is_some_and(|m| drops(m.kind, r));
            if drop {
                return false;
            }
            if let Some(props) = run.direct_props.as_mut() {
                props.revision = None;
            }
            resolve_inlines(&mut run.content, r);
            true
        }
        Inline::Emph(c)
        | Inline::Underline(c)
        | Inline::Strong(c)
        | Inline::Strikeout(c)
        | Inline::Superscript(c)
        | Inline::Subscript(c)
        | Inline::SmallCaps(c)
        | Inline::Quoted(_, c)
        | Inline::Span(_, c)
        | Inline::Link(_, c, _)
        | Inline::Image(_, c, _)
        | Inline::Cite(_, c) => {
            resolve_inlines(c, r);
            true
        }
        Inline::Note(_, blocks) => {
            resolve_blocks(blocks, r);
            true
        }
        _ => true,
    }
}

fn resolve_inlines(inlines: &mut Vec<Inline>, r: Resolution) {
    inlines.retain_mut(|i| resolve_inline(i, r));
}

fn resolve_rows(rows: &mut [Row], r: Resolution) {
    for row in rows {
        for cell in &mut row.cells {
            resolve_blocks(&mut cell.blocks, r);
        }
    }
}

fn resolve_table(table: &mut Table, r: Resolution) {
    resolve_rows(&mut table.head.rows, r);
    for body in &mut table.bodies {
        resolve_rows(&mut body.head_rows, r);
        resolve_rows(&mut body.body_rows, r);
    }
    resolve_rows(&mut table.foot.rows, r);
}

fn resolve_block(block: &mut Block, r: Resolution) {
    match block {
        Block::Para(i) | Block::Plain(i) | Block::Heading(_, _, i) => resolve_inlines(i, r),
        Block::StyledPara(p) => resolve_inlines(&mut p.inlines, r),
        Block::LineBlock(lines) => lines.iter_mut().for_each(|l| resolve_inlines(l, r)),
        Block::BlockQuote(bs) | Block::Div(_, bs) | Block::Figure(_, _, bs) => {
            resolve_blocks(bs, r)
        }
        Block::BulletList(items) | Block::OrderedList(_, items) => {
            items.iter_mut().for_each(|it| resolve_blocks(it, r));
        }
        Block::DefinitionList(items) => {
            for (term, defs) in items {
                resolve_inlines(term, r);
                defs.iter_mut().for_each(|d| resolve_blocks(d, r));
            }
        }
        Block::Table(t) => resolve_table(t, r),
        Block::TableOfContents(toc) => resolve_blocks(&mut toc.body, r),
        Block::Index(ix) => resolve_blocks(&mut ix.body, r),
        _ => {}
    }
}

fn resolve_blocks(blocks: &mut Vec<Block>, r: Resolution) {
    for b in blocks.iter_mut() {
        resolve_block(b, r);
    }
    // A struck paragraph mark (¶) merges/clears at the block-list level.
    super::para_mark_merge::resolve_para_marks(blocks, matches!(r, Resolution::Accept));
}

/// Accepts every tracked change in `blocks`: insertions become permanent (mark
/// cleared) and deletions are removed (a struck paragraph mark merges).
pub fn accept_revisions(blocks: &mut Vec<Block>) {
    resolve_blocks(blocks, Resolution::Accept);
}

/// Rejects every tracked change in `blocks`: insertions are removed and
/// deletions are restored (mark cleared).
pub fn reject_revisions(blocks: &mut Vec<Block>) {
    resolve_blocks(blocks, Resolution::Reject);
}

/// Whether any run in `blocks` carries a tracked-change mark.
#[must_use]
pub fn has_revisions(blocks: &[Block]) -> bool {
    fn inline_has(i: &Inline) -> bool {
        match i {
            Inline::StyledRun(run) => {
                run.direct_props
                    .as_ref()
                    .is_some_and(|p| p.revision.is_some())
                    || run.content.iter().any(inline_has)
            }
            Inline::Emph(c)
            | Inline::Underline(c)
            | Inline::Strong(c)
            | Inline::Strikeout(c)
            | Inline::Superscript(c)
            | Inline::Subscript(c)
            | Inline::SmallCaps(c)
            | Inline::Quoted(_, c)
            | Inline::Span(_, c)
            | Inline::Link(_, c, _)
            | Inline::Image(_, c, _)
            | Inline::Cite(_, c) => c.iter().any(inline_has),
            Inline::Note(_, bs) => has_revisions(bs),
            _ => false,
        }
    }
    fn rows_have(rows: &[Row]) -> bool {
        rows.iter()
            .any(|row| row.cells.iter().any(|c| has_revisions(&c.blocks)))
    }
    blocks.iter().any(|block| match block {
        Block::Para(i) | Block::Plain(i) | Block::Heading(_, _, i) => i.iter().any(inline_has),
        Block::StyledPara(p) => {
            p.inlines.iter().any(inline_has)
                || p.direct_char_props
                    .as_ref()
                    .and_then(|c| c.revision.as_ref())
                    .is_some()
        }
        Block::LineBlock(lines) => lines.iter().flatten().any(inline_has),
        Block::BlockQuote(bs) | Block::Div(_, bs) | Block::Figure(_, _, bs) => has_revisions(bs),
        Block::BulletList(items) | Block::OrderedList(_, items) => {
            items.iter().any(|it| has_revisions(it))
        }
        Block::DefinitionList(items) => items
            .iter()
            .any(|(t, defs)| t.iter().any(inline_has) || defs.iter().any(|d| has_revisions(d))),
        Block::Table(t) => {
            rows_have(&t.head.rows)
                || rows_have(&t.foot.rows)
                || t.bodies
                    .iter()
                    .any(|b| rows_have(&b.head_rows) || rows_have(&b.body_rows))
        }
        Block::TableOfContents(toc) => has_revisions(&toc.body),
        Block::Index(ix) => has_revisions(&ix.body),
        _ => false,
    })
}

impl Document {
    /// Accepts every tracked change across all sections.
    pub fn accept_all_revisions(&mut self) {
        for section in &mut self.sections {
            accept_revisions(&mut section.blocks);
        }
    }

    /// Rejects every tracked change across all sections.
    pub fn reject_all_revisions(&mut self) {
        for section in &mut self.sections {
            reject_revisions(&mut section.blocks);
        }
    }

    /// Whether the document contains any tracked change.
    #[must_use]
    pub fn has_tracked_changes(&self) -> bool {
        self.sections.iter().any(|s| has_revisions(&s.blocks))
    }

    /// The revision mark to stamp on newly typed text when **track changes** is
    /// on (`DocumentSettings::track_changes`) — an insertion attributed to the
    /// document's author (`meta.creator`) — or `None` when tracking is off. The
    /// editor routes typing through `insert_text_tracked_at` with this mark.
    #[must_use]
    pub fn insertion_revision(&self) -> Option<crate::style::props::revision::RevisionMark> {
        self.author_revision(RevisionKind::Insertion)
    }

    /// The revision mark for a **tracked deletion** by the document's author when
    /// track changes is on (else `None`) — applied to text struck out by
    /// Backspace/Delete. Its `Some`/`None` also tells the editor whether tracking
    /// is on (drives [`delete_action`]).
    #[must_use]
    pub fn deletion_revision(&self) -> Option<crate::style::props::revision::RevisionMark> {
        self.author_revision(RevisionKind::Deletion)
    }

    /// A revision mark of `kind` attributed to `meta.creator` when track changes
    /// is on; else `None`.
    fn author_revision(
        &self,
        kind: RevisionKind,
    ) -> Option<crate::style::props::revision::RevisionMark> {
        use crate::style::props::revision::RevisionMark;
        self.settings.as_ref().filter(|s| s.track_changes).map(|_| {
            let mut mark = RevisionMark::new(kind);
            mark.author = self.meta.creator.clone();
            mark
        })
    }
}

#[cfg(test)]
#[path = "revision_ops_tests.rs"]
mod tests;
