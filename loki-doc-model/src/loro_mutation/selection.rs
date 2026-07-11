// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Selection deletion: removing a contiguous text range that may span several
//! sibling blocks (Spec/audit F6c — typing and Backspace must operate on the
//! active selection).
//!
//! The range is expressed as two `(BlockPath, byte_offset)` endpoints in
//! either order. Both endpoints must lie in the **same container** (the same
//! top-level section space, the same table cell, or the same note body) —
//! a selection that crosses a container boundary (body ↔ cell, cell ↔ cell)
//! is rejected with [`MutationError::CrossContainerSelection`], mirroring the
//! formatting layer's clamping rule.
//!
//! A multi-block deletion is composed from the existing primitives:
//! every following block in the range is [`merge_block_at`]-ed into the
//! first (the merge concatenates text and removes the merged block), and one
//! [`delete_text_at`] then removes `[start_byte, join + end_byte)` from the
//! merged text — the tail of the first block, every middle block's text, and
//! the head of the last. Word's behaviour falls out naturally: the surviving
//! paragraph keeps the *first* block's style.

use loro::{ContainerTrait, LoroDoc, LoroText, LoroValue, TextDelta};

use super::nested::{BlockPath, PathStep, resolve_block_list, text_for_path};
use super::{MutationError, delete_text_at, merge_block_at};
use crate::content::revision_ops::{DeleteAction, delete_action};
use crate::loro_schema::MARK_REVISION;
use crate::style::props::revision::{RevisionMark, decode, encode};

/// The block index of `path`'s leaf within its container (the root index for
/// a top-level path, the leaf step's block index for a nested one).
fn leaf_index(path: &BlockPath) -> usize {
    match path.steps.last() {
        Some(PathStep::Cell { block, .. } | PathStep::Note { block, .. }) => *block,
        None => path.root,
    }
}

/// `path` with its leaf block index replaced by `leaf`.
fn with_leaf(path: &BlockPath, leaf: usize) -> BlockPath {
    let mut p = path.clone();
    match p.steps.last_mut() {
        Some(PathStep::Cell { block, .. } | PathStep::Note { block, .. }) => *block = leaf,
        None => p.root = leaf,
    }
    p
}

/// Validates that `byte` is a UTF-8 char boundary within the text of the block
/// at `path` (rejecting a stale offset before any mutation runs).
fn validate_offset(loro: &LoroDoc, path: &BlockPath, byte: usize) -> Result<(), MutationError> {
    let text = text_for_path(loro, path)?.to_string();
    if byte > text.len() || !text.is_char_boundary(byte) {
        return Err(MutationError::InvalidByteOffset { offset: byte });
    }
    Ok(())
}

/// Whether two paths address sibling blocks of one container: both top-level,
/// or nested with identical root, identical non-leaf steps, and the same leaf
/// cell / note.
fn same_container(a: &BlockPath, b: &BlockPath) -> bool {
    if a.steps.len() != b.steps.len() {
        return false;
    }
    let Some(n) = a.steps.len().checked_sub(1) else {
        return true; // both top-level; cross-section is caught by list identity
    };
    if a.root != b.root || a.steps[..n] != b.steps[..n] {
        return false;
    }
    match (a.steps[n], b.steps[n]) {
        (PathStep::Cell { cell: c1, .. }, PathStep::Cell { cell: c2, .. }) => c1 == c2,
        (PathStep::Note { note: n1, .. }, PathStep::Note { note: n2, .. }) => n1 == n2,
        _ => false,
    }
}

/// A selection's two endpoints in document order within one container. Each is a
/// `(path, byte_offset, leaf_index)` triple with a validated char-boundary offset.
type Ordered<'p> = ((&'p BlockPath, usize, usize), (&'p BlockPath, usize, usize));

/// Validates the two endpoints share a container and normalizes them into
/// document order (returning each with its leaf block index).
///
/// The byte offsets are validated against the actual block text lengths BEFORE
/// any mutation: a stale offset (e.g. a concurrent remote edit that shortened a
/// paragraph) is rejected up front instead of surfacing mid-way through a
/// multi-block delete and leaving the document half-applied.
fn normalize_selection<'p>(
    loro: &LoroDoc,
    a: (&'p BlockPath, usize),
    b: (&'p BlockPath, usize),
) -> Result<Ordered<'p>, MutationError> {
    if !same_container(a.0, b.0) {
        return Err(MutationError::CrossContainerSelection);
    }
    let ((sp, sb), (ep, eb)) = if (leaf_index(a.0), a.1) <= (leaf_index(b.0), b.1) {
        (a, b)
    } else {
        (b, a)
    };
    validate_offset(loro, sp, sb)?;
    validate_offset(loro, ep, eb)?;
    Ok(((sp, sb, leaf_index(sp)), (ep, eb, leaf_index(ep))))
}

/// Deletes the text between two positions (in either order), collapsing any
/// blocks the range spans. Returns the collapsed cursor position — the
/// ordered start endpoint.
///
/// # Errors
///
/// - [`MutationError::CrossContainerSelection`] — the endpoints live in
///   different containers (or different sections); nothing is mutated.
/// - [`MutationError::TextNotFound`] — a block inside the range carries no
///   editable text (e.g. a table or horizontal rule); nothing is mutated
///   (the whole range is validated before the first mutation).
/// - [`MutationError::InvalidByteOffset`] — an endpoint offset is past the end
///   of its block's text or not on a char boundary (e.g. a stale offset from a
///   concurrent edit); nothing is mutated.
/// - Any error the underlying primitives report.
pub fn delete_selection_at(
    loro: &LoroDoc,
    a: (&BlockPath, usize),
    b: (&BlockPath, usize),
) -> Result<(BlockPath, usize), MutationError> {
    let ((start_path, start_byte, start_leaf), (end_path, end_byte, end_leaf)) =
        normalize_selection(loro, a, b)?;

    // Single-block selection: one plain text deletion.
    if start_leaf == end_leaf {
        if end_byte > start_byte {
            delete_text_at(loro, start_path, start_byte, end_byte - start_byte)?;
        }
        return Ok((start_path.clone(), start_byte));
    }

    // Validate the whole range BEFORE mutating, so unsupported content inside
    // the selection (a table, a rule, a section break) rejects the operation
    // instead of leaving it half-applied.
    let (start_list, _) = resolve_block_list(loro, start_path)?;
    let (end_list, _) = resolve_block_list(loro, end_path)?;
    if start_list.id() != end_list.id() {
        return Err(MutationError::CrossContainerSelection);
    }
    for leaf in start_leaf..=end_leaf {
        text_for_path(loro, &with_leaf(start_path, leaf))?;
    }

    // Merge every following block in the range into the start block. Each
    // merge removes the merged block, so the next one is always at
    // `start_leaf + 1`. The final join offset is where the end block's text
    // begins within the merged text.
    let merge_path = with_leaf(start_path, start_leaf + 1);
    let mut join = 0usize;
    for _ in start_leaf..end_leaf {
        join = merge_block_at(loro, &merge_path)?;
    }
    delete_text_at(loro, start_path, start_byte, join + end_byte - start_byte)?;
    Ok((start_path.clone(), start_byte))
}

/// Deletes the selection under **track changes** (Review tab, 4a.2): instead of
/// removing text, each selected run is struck through (a `MARK_REVISION`
/// deletion) — except the author's own tracked insertions, which are
/// hard-deleted (un-typed), and already-struck text, which is skipped
/// ([`delete_action`]). Block boundaries are **preserved** (no merge), so the
/// paragraph marks between selected blocks survive; deleting a paragraph mark
/// itself is not modelled (`TODO(review-selection-delete)`).
///
/// With `deletion` = `None` (tracking off) this is exactly
/// [`delete_selection_at`]. Returns the collapsed cursor at the selection start.
///
/// # Errors
///
/// Same as [`delete_selection_at`] (cross-container / stale-offset / non-text
/// block inside the range are all rejected before any mutation).
pub fn tracked_delete_selection_at(
    loro: &LoroDoc,
    a: (&BlockPath, usize),
    b: (&BlockPath, usize),
    deletion: Option<&RevisionMark>,
) -> Result<(BlockPath, usize), MutationError> {
    let Some(mark) = deletion else {
        return delete_selection_at(loro, a, b);
    };
    let ((start_path, start_byte, start_leaf), (end_path, end_byte, end_leaf)) =
        normalize_selection(loro, a, b)?;

    // Single-block selection: strike its `[start_byte, end_byte)`.
    if start_leaf == end_leaf {
        let text = text_for_path(loro, start_path)?;
        strike_range(&text, start_byte, end_byte, mark)?;
        return Ok((start_path.clone(), start_byte));
    }

    // Validate the whole range up front (same list, every block has text) so an
    // unsupported block inside the selection rejects before any mutation.
    let (start_list, _) = resolve_block_list(loro, start_path)?;
    let (end_list, _) = resolve_block_list(loro, end_path)?;
    if start_list.id() != end_list.id() {
        return Err(MutationError::CrossContainerSelection);
    }
    for leaf in start_leaf..=end_leaf {
        text_for_path(loro, &with_leaf(start_path, leaf))?;
    }

    // Strike each block's slice: the first block's tail, every middle block in
    // full, and the last block's head. No merge, so the paragraph marks stay.
    for leaf in start_leaf..=end_leaf {
        let text = text_for_path(loro, &with_leaf(start_path, leaf))?;
        let (lo, hi) = if leaf == start_leaf {
            (start_byte, text.len_utf8())
        } else if leaf == end_leaf {
            (0, end_byte)
        } else {
            (0, text.len_utf8())
        };
        strike_range(&text, lo, hi, mark)?;
    }
    Ok((start_path.clone(), start_byte))
}

/// Strikes the run segments intersecting `[range_start, range_end)` in one text
/// container as tracked deletions, applying [`delete_action`] per segment: the
/// author's own tracked insertion is hard-deleted, an already-struck deletion is
/// left alone, and everything else is marked struck with `mark`. Ops are applied
/// back-to-front so a hard delete never shifts an earlier segment's offset.
fn strike_range(
    text: &LoroText,
    range_start: usize,
    range_end: usize,
    mark: &RevisionMark,
) -> Result<(), MutationError> {
    // Collect (lo, hi, action) for each marked segment before mutating.
    let mut ops: Vec<(usize, usize, DeleteAction)> = Vec::new();
    let mut byte_pos = 0usize;
    for delta in text.to_delta() {
        if let TextDelta::Insert { insert, attributes } = delta {
            let (seg_start, seg_end) = (byte_pos, byte_pos + insert.len());
            byte_pos = seg_end;
            let (lo, hi) = (seg_start.max(range_start), seg_end.min(range_end));
            if lo >= hi {
                continue;
            }
            let existing = attributes
                .as_ref()
                .and_then(|a| a.get(MARK_REVISION))
                .and_then(|v| match v {
                    LoroValue::String(s) => decode(s.as_str()),
                    _ => None,
                })
                .map(|m| m.kind);
            let action = delete_action(existing, true);
            if !matches!(action, DeleteAction::Skip) {
                ops.push((lo, hi, action));
            }
        }
    }
    let encoded = encode(mark);
    for (lo, hi, action) in ops.into_iter().rev() {
        match action {
            DeleteAction::HardDelete => text.delete_utf8(lo, hi - lo)?,
            DeleteAction::MarkDeleted => {
                text.mark_utf8(lo..hi, MARK_REVISION, LoroValue::from(encoded.clone()))?;
            }
            DeleteAction::Skip => {}
        }
    }
    Ok(())
}
