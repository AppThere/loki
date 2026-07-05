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

use loro::{ContainerTrait, LoroDoc};

use super::nested::{BlockPath, PathStep, resolve_block_list, text_for_path};
use super::{MutationError, delete_text_at, merge_block_at};

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
/// - Any error the underlying primitives report.
pub fn delete_selection_at(
    loro: &LoroDoc,
    a: (&BlockPath, usize),
    b: (&BlockPath, usize),
) -> Result<(BlockPath, usize), MutationError> {
    if !same_container(a.0, b.0) {
        return Err(MutationError::CrossContainerSelection);
    }
    // Normalize the endpoints into document order within the container.
    let ((start_path, start_byte), (end_path, end_byte)) =
        if (leaf_index(a.0), a.1) <= (leaf_index(b.0), b.1) {
            (a, b)
        } else {
            (b, a)
        };
    let start_leaf = leaf_index(start_path);
    let end_leaf = leaf_index(end_path);

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
