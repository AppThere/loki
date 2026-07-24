// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Applies a macro's [`EditBatch`] to the live Loro document as **one undo
//! entry** (macro spec §6.2).
//!
//! Every edit the macro made is applied with the same section-0 text/block
//! primitives the editor uses per keystroke, then committed **exactly once** —
//! the editor commits once per user action and each commit is a single undo
//! checkpoint, so a whole macro run collapses to one ⌘Z. A runaway-but-permitted
//! macro is therefore recoverable with a single undo (spec §6.2).
//!
//! The macro object model is plain-text (v1): the document body is the section-0
//! blocks (one paragraph each). `AppendText` extends the last paragraph;
//! `SetText` replaces the whole body with a single paragraph. Rich structure
//! (multiple sections, per-run formatting on inserted text) is out of scope for
//! v1 — see `docs/fidelity-status.md §12`.

use std::sync::{Arc, Mutex};

use loki_doc_model::{MutationError, delete_block, delete_text, get_block_text, insert_text};
use loki_macro_host::{DocEdit, EditBatch};

use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Applies `batch` to `loro` (section-0 blocks), commits once, and relays out.
///
/// Returns `Ok(true)` when at least one edit was applied (so the caller can
/// treat it as a document change), `Ok(false)` for an empty batch (no commit,
/// no undo entry). On a mutation error the partial edits are **not** committed
/// — the caller sees the error and the document is left untouched by undo.
///
/// # Errors
///
/// Propagates any [`MutationError`] from the underlying Loro primitives.
pub(super) fn apply_edit_batch(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro: &loro::LoroDoc,
    batch: &EditBatch,
) -> Result<bool, MutationError> {
    if batch.is_empty() {
        return Ok(false);
    }
    let block_count = section0_block_count(doc_state);
    apply_batch_ops(loro, block_count, batch)?;
    // One commit == one undo checkpoint == one undo entry (spec §6.2).
    loro.commit();
    apply_mutation_and_relayout(doc_state, loro);
    Ok(true)
}

/// The number of top-level blocks in section 0 of the currently-published
/// document (≥ 1 for any real document; `1` as a safe fallback).
fn section0_block_count(doc_state: &Arc<Mutex<DocumentState>>) -> usize {
    doc_state
        .lock()
        .ok()
        .and_then(|s| {
            s.document
                .as_ref()
                .and_then(|d| d.sections.first())
                .map(|sec| sec.blocks.len())
        })
        .unwrap_or(1)
        .max(1)
}

/// Translates `batch` into section-0 Loro text/block mutations, **without**
/// committing. Split out so it can be unit-tested against a bare `LoroDoc`.
///
/// `block_count` is the section-0 block count before the batch. It is tracked
/// across edits (only `SetText` changes it — collapsing the body to one block).
///
/// # Errors
///
/// Propagates any [`MutationError`] from the Loro primitives.
pub(super) fn apply_batch_ops(
    loro: &loro::LoroDoc,
    block_count: usize,
    batch: &EditBatch,
) -> Result<(), MutationError> {
    let mut blocks = block_count.max(1);
    for edit in &batch.edits {
        match edit {
            DocEdit::AppendText(s) => {
                if s.is_empty() {
                    continue;
                }
                let last = blocks - 1;
                let offset = get_block_text(loro, last).len();
                insert_text(loro, last, offset, s)?;
            }
            DocEdit::SetText(s) => {
                // Collapse the body to a single paragraph holding `s`: remove the
                // trailing blocks (from the end so indices stay valid), then
                // replace block 0's text.
                for idx in (1..blocks).rev() {
                    delete_block(loro, idx)?;
                }
                let existing = get_block_text(loro, 0);
                delete_text(loro, 0, 0, existing.len())?;
                if !s.is_empty() {
                    insert_text(loro, 0, 0, s)?;
                }
                blocks = 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "editor_macro_apply_tests.rs"]
mod tests;
