// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tracked-change CRDT mutations (Review tab, 4a.2): accept/reject all, and the
//! tracked grapheme delete that Backspace/Delete route through.
//!
//! The pure [`accept_revisions`][crate::content::revision_ops::accept_revisions]
//! transforms operate on a rebuilt [`Document`][crate::document::Document]; this
//! applies the same semantics **surgically** to the Loro text so the edit is one
//! undoable step and the editor keeps its live document handle:
//!
//! - a kept run (accepted insertion / rejected deletion) has its `MARK_REVISION`
//!   cleared (`mark_utf8(range, …, Null)`) — the text stays, un-tracked;
//! - a removed run (accepted deletion / rejected insertion) has its text deleted.
//!
//! Scope: every text container in the document — the accept/reject-all sweep
//! descends into table cells and note bodies, and the per-change ops are
//! path-aware, so a revision anywhere in the tree resolves.

use loro::{LoroDoc, LoroText, LoroValue, TextDelta};

use super::nested::text_for_path;
use super::text_containers::collect_all_text_containers;
use super::{BlockPath, MutationError, delete_text_at, get_mark_at_path, mark_text_at};
use crate::content::revision_ops::{DeleteAction, delete_action};
use crate::loro_schema::MARK_REVISION;
use crate::style::props::revision::{RevisionKind, RevisionMark, decode, encode};

/// Whether a revision run of `kind` is **removed** (vs. kept, mark cleared) when
/// resolving with `accept` — the CRDT analogue of `revision_ops::drops`.
fn removes(kind: RevisionKind, accept: bool) -> bool {
    matches!(
        (accept, kind),
        (true, RevisionKind::Deletion) | (false, RevisionKind::Insertion)
    )
}

/// Collects every `MARK_REVISION` span in one text container as
/// `(byte_start, byte_len, kind)`, in document order.
fn revision_spans(text: &LoroText) -> Vec<(usize, usize, RevisionKind)> {
    let mut spans = Vec::new();
    let mut byte_pos = 0usize;
    for delta in text.to_delta() {
        if let TextDelta::Insert { insert, attributes } = delta {
            let span_bytes = insert.len();
            if let Some(attrs) = attributes
                && let Some(LoroValue::String(s)) = attrs.get(MARK_REVISION)
                && let Some(mark) = decode(s.as_str())
            {
                spans.push((byte_pos, span_bytes, mark.kind));
            }
            byte_pos += span_bytes;
        }
    }
    spans
}

/// Applies the accept/reject resolution to one revision span: a removed run's
/// text is deleted, a kept run's mark is cleared (the text stays, un-tracked).
fn resolve_span(
    text: &LoroText,
    start: usize,
    len: usize,
    kind: RevisionKind,
    accept: bool,
) -> Result<(), MutationError> {
    if removes(kind, accept) {
        text.delete_utf8(start, len)?;
    } else {
        text.mark_utf8(start..start + len, MARK_REVISION, LoroValue::Null)?;
    }
    Ok(())
}

/// Resolves every `MARK_REVISION` span in one text container, returning how many
/// were resolved. Ops are applied back-to-front so a delete never shifts an
/// earlier span's byte offset.
fn resolve_text(text: &LoroText, accept: bool) -> Result<usize, MutationError> {
    let spans = revision_spans(text);
    let count = spans.len();
    for (start, len, kind) in spans.into_iter().rev() {
        resolve_span(text, start, len, kind, accept)?;
    }
    Ok(count)
}

/// The tracked-change span at `byte_offset` — the marked span containing it, or
/// (failing that) the one ending exactly at it (a caret just past a change).
fn span_at(
    spans: &[(usize, usize, RevisionKind)],
    byte_offset: usize,
) -> Option<(usize, usize, RevisionKind)> {
    spans
        .iter()
        .copied()
        .find(|&(s, l, _)| s <= byte_offset && byte_offset < s + l)
        .or_else(|| {
            spans
                .iter()
                .copied()
                .find(|&(s, l, _)| s + l == byte_offset)
        })
}

/// Whether a single tracked change sits at `byte_offset` in the block at `path`
/// (drives the per-change Accept/Reject buttons' enabled state).
#[must_use]
pub fn revision_at(loro: &LoroDoc, path: &BlockPath, byte_offset: usize) -> bool {
    text_for_path(loro, path)
        .map(|t| span_at(&revision_spans(&t), byte_offset).is_some())
        .unwrap_or(false)
}

/// Accepts (`accept = true`) or rejects the single tracked change at
/// `byte_offset` in the block at `path` (Review tab, 4a.2). Resolves the
/// contiguous `MARK_REVISION` span at the caret — the unit the editor records
/// (consecutive same-author edits coalesce into one span).
///
/// Returns the collapsed caret offset when a change was resolved (the change
/// start if its text was removed, else the offset unchanged), or `None` when the
/// caret is not on a change.
///
/// # Errors
///
/// [`MutationError`] for an underlying path / Loro error.
pub fn accept_reject_revision_at(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_offset: usize,
    accept: bool,
) -> Result<Option<usize>, MutationError> {
    let text = text_for_path(loro, path)?;
    let Some((start, len, kind)) = span_at(&revision_spans(&text), byte_offset) else {
        return Ok(None);
    };
    resolve_span(&text, start, len, kind, accept)?;
    Ok(Some(if removes(kind, accept) {
        start
    } else {
        byte_offset
    }))
}

/// Accepts (`accept = true`) or rejects (`false`) **every** tracked change in the
/// document, returning the number of change runs resolved. Sweeps every text
/// container — top-level blocks and those nested in table cells / note bodies.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn accept_reject_all_revisions(loro: &LoroDoc, accept: bool) -> Result<usize, MutationError> {
    let mut total = 0usize;
    for text in collect_all_text_containers(loro) {
        total += resolve_text(&text, accept)?;
    }
    Ok(total)
}

/// Deletes the grapheme `byte_start..byte_end` in `path`, honouring track
/// changes (Review tab, 4a.2), and returns what it did so the editor can place
/// the caret. `deletion` is the tracked-deletion mark to apply (its `Some`/`None`
/// is whether tracking is on — see [`Document::deletion_revision`]).
///
/// - the author's own tracked **insertion** is hard-deleted (un-typed);
/// - an already-struck **deletion** is skipped (no mutation);
/// - otherwise the range is marked struck ([`DeleteAction::MarkDeleted`]);
/// - with tracking off, always a hard delete.
///
/// [`Document::deletion_revision`]: crate::document::Document::deletion_revision
///
/// # Errors
///
/// [`MutationError::Loro`] / [`MutationError::InvalidBlockPath`] for underlying
/// path/Loro errors.
pub fn tracked_grapheme_delete(
    loro: &LoroDoc,
    path: &BlockPath,
    byte_start: usize,
    byte_end: usize,
    deletion: Option<&RevisionMark>,
) -> Result<DeleteAction, MutationError> {
    if byte_start >= byte_end {
        return Ok(DeleteAction::Skip);
    }
    let existing = get_mark_at_path(loro, path, byte_start, MARK_REVISION)?
        .and_then(|v| match v {
            LoroValue::String(s) => decode(&s),
            _ => None,
        })
        .map(|m| m.kind);
    let action = delete_action(existing, deletion.is_some());
    match action {
        DeleteAction::HardDelete => delete_text_at(loro, path, byte_start, byte_end - byte_start)?,
        DeleteAction::MarkDeleted => {
            if let Some(mark) = deletion {
                mark_text_at(
                    loro,
                    path,
                    byte_start,
                    byte_end,
                    MARK_REVISION,
                    LoroValue::from(encode(mark)),
                )?;
            }
        }
        DeleteAction::Skip => {}
    }
    Ok(action)
}

#[cfg(test)]
#[path = "revision_tests.rs"]
mod tests;
