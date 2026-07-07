// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Accept / reject tracked changes on the live CRDT (Review tab, 4a.2).
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
//! Scope: top-level block text across every section. Revisions nested in table
//! cells / note bodies are not yet resolved here — `TODO(review-nested)`.

use loro::{LoroDoc, LoroText, LoroValue, TextDelta};

use super::{MutationError, get_loro_text_for_block};
use crate::loro_schema::MARK_REVISION;
use crate::style::props::revision::{RevisionKind, decode};

/// Whether a revision run of `kind` is **removed** (vs. kept, mark cleared) when
/// resolving with `accept` — the CRDT analogue of `revision_ops::drops`.
fn removes(kind: RevisionKind, accept: bool) -> bool {
    matches!(
        (accept, kind),
        (true, RevisionKind::Deletion) | (false, RevisionKind::Insertion)
    )
}

/// Resolves every `MARK_REVISION` span in one text container, returning how many
/// were resolved. Ops are applied back-to-front so a delete never shifts an
/// earlier span's byte offset.
fn resolve_text(text: &LoroText, accept: bool) -> Result<usize, MutationError> {
    // Collect (byte_start, byte_len, kind) for each revision-marked span.
    let mut ops: Vec<(usize, usize, RevisionKind)> = Vec::new();
    let mut byte_pos = 0usize;
    for delta in text.to_delta() {
        if let TextDelta::Insert { insert, attributes } = delta {
            let span_bytes = insert.len();
            if let Some(attrs) = attributes
                && let Some(LoroValue::String(s)) = attrs.get(MARK_REVISION)
                && let Some(mark) = decode(s.as_str())
            {
                ops.push((byte_pos, span_bytes, mark.kind));
            }
            byte_pos += span_bytes;
        }
    }
    let count = ops.len();
    for (start, len, kind) in ops.into_iter().rev() {
        if removes(kind, accept) {
            text.delete_utf8(start, len)?;
        } else {
            text.mark_utf8(start..start + len, MARK_REVISION, LoroValue::Null)?;
        }
    }
    Ok(count)
}

/// Accepts (`accept = true`) or rejects (`false`) **every** tracked change in the
/// document's top-level block text, returning the number of change runs resolved.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn accept_reject_all_revisions(loro: &LoroDoc, accept: bool) -> Result<usize, MutationError> {
    let mut total = 0usize;
    let mut idx = 0usize;
    loop {
        match get_loro_text_for_block(loro, idx) {
            Ok(text) => total += resolve_text(&text, accept)?,
            // A table / stub block has no top-level text — skip it (nested-cell
            // revisions are TODO(review-nested)).
            Err(MutationError::TextNotFound(_)) => {}
            Err(MutationError::BlockIndexOutOfRange(_)) => break,
            Err(e) => return Err(e),
        }
        idx += 1;
    }
    Ok(total)
}

#[cfg(test)]
#[path = "revision_tests.rs"]
mod tests;
