// SPDX-License-Identifier: Apache-2.0

//! Format-range resolution: maps the cursor/selection to the per-paragraph
//! byte ranges a character-formatting action applies to (plan 4b.2).
//!
//! Extracted from `editor_formatting.rs` to keep that file under the
//! 300-line ceiling.
//!
//! # Byte offset coordinate space
//!
//! All byte offsets are UTF-8 byte positions, matching `CursorState` and the
//! `mark_utf8` API used by `mark_text_at`.

use loki_doc_model::{BlockPath, PathStep, get_block_text_at};
use loro::LoroDoc;

use crate::editing::cursor::{CursorState, DocumentPosition};

#[cfg(test)]
#[path = "editor_format_range_tests.rs"]
mod tests;

/// Resolves the format ranges from cursor state, one `(BlockPath, byte_start,
/// byte_end)` per paragraph the action applies to, in document order.
///
/// - Selection within one paragraph → that byte range.
/// - Selection spanning sibling paragraphs of one container (top-level
///   blocks, or blocks of the same table cell / note body) → the tail of the
///   first paragraph, every middle paragraph in full, and the head of the
///   last. Non-text blocks inside the range (e.g. a table between two
///   top-level paragraphs) contribute an empty range and are skipped, so
///   their nested content is left untouched.
/// - Selection crossing containers (body ↔ cell, cell ↔ cell) → clamped to
///   the focus paragraph, mirroring the model layer's cross-container
///   rejection rule.
/// - Point cursor → the word at the cursor.
///
/// Empty ranges are never returned; an empty `Vec` means there is nothing to
/// format.
pub fn resolve_format_ranges(
    loro: &LoroDoc,
    cursor: &CursorState,
) -> Vec<(BlockPath, usize, usize)> {
    let Some(focus) = cursor.focus.as_ref() else {
        return Vec::new();
    };
    let path = focus.block_path();

    if cursor.has_selection() {
        let Some(anchor) = cursor.anchor.as_ref() else {
            return Vec::new();
        };
        // Same paragraph requires the same index *and* the same nesting path
        // (two cells of one table share the root index but differ by path).
        if anchor.paragraph_index == focus.paragraph_index && anchor.path == focus.path {
            let (start, end) = if anchor.byte_offset <= focus.byte_offset {
                (anchor.byte_offset, focus.byte_offset)
            } else {
                (focus.byte_offset, anchor.byte_offset)
            };
            if start < end {
                return vec![(path, start, end)];
            }
            return Vec::new();
        }
        if same_container(anchor, focus) {
            return sibling_ranges(loro, anchor, focus);
        }
        if focus.byte_offset > 0 {
            // Cross-container: clamp to the focus paragraph as a best-effort.
            return vec![(path, 0, focus.byte_offset)];
        }
        return Vec::new();
    }

    // No selection — expand to the word at cursor.
    let text = get_block_text_at(loro, &path);
    let (word_start, word_end) = word_bounds_at(&text, focus.byte_offset);
    if word_start < word_end {
        vec![(path, word_start, word_end)]
    } else {
        Vec::new()
    }
}

/// Resolves a single format range — the first of [`resolve_format_ranges`].
///
/// Used by actions that need one contiguous range (e.g. hyperlink insertion);
/// for a multi-paragraph selection this is the selection's portion of its
/// first paragraph.
pub fn resolve_format_range(
    loro: &LoroDoc,
    cursor: &CursorState,
) -> Option<(BlockPath, usize, usize)> {
    resolve_format_ranges(loro, cursor).into_iter().next()
}

/// The leaf block index of a position within its container (the leaf step's
/// block index for nested positions, the paragraph index for top-level ones).
fn leaf_index(pos: &DocumentPosition) -> usize {
    match pos.path.last() {
        Some(PathStep::Cell { block, .. } | PathStep::Note { block, .. }) => *block,
        None => pos.paragraph_index,
    }
}

/// Whether two positions address sibling blocks of one container: both
/// top-level, or nested with the same root, the same non-leaf steps, and the
/// same leaf cell / note (mirrors `loro_mutation::selection`).
fn same_container(a: &DocumentPosition, b: &DocumentPosition) -> bool {
    if a.path.len() != b.path.len() {
        return false;
    }
    let Some(n) = a.path.len().checked_sub(1) else {
        return true; // both top-level
    };
    if a.paragraph_index != b.paragraph_index || a.path[..n] != b.path[..n] {
        return false;
    }
    match (a.path[n], b.path[n]) {
        (PathStep::Cell { cell: c1, .. }, PathStep::Cell { cell: c2, .. }) => c1 == c2,
        (PathStep::Note { note: n1, .. }, PathStep::Note { note: n2, .. }) => n1 == n2,
        _ => false,
    }
}

/// Per-paragraph ranges for a selection spanning sibling blocks of one
/// container: `[start_byte..len]` of the first, `[0..len]` of every middle
/// block, `[0..end_byte]` of the last. Empty ranges (empty paragraphs,
/// text-less blocks like tables) are skipped.
fn sibling_ranges(
    loro: &LoroDoc,
    a: &DocumentPosition,
    b: &DocumentPosition,
) -> Vec<(BlockPath, usize, usize)> {
    let (start, end) = if (leaf_index(a), a.byte_offset) <= (leaf_index(b), b.byte_offset) {
        (a, b)
    } else {
        (b, a)
    };
    let (start_leaf, end_leaf) = (leaf_index(start), leaf_index(end));

    let mut ranges = Vec::with_capacity(end_leaf - start_leaf + 1);
    for leaf in start_leaf..=end_leaf {
        let pos = start.sibling_block(leaf as isize - start_leaf as isize, 0);
        let bp = pos.block_path();
        let text_len = get_block_text_at(loro, &bp).len();
        let s = if leaf == start_leaf {
            start.byte_offset
        } else {
            0
        };
        let e = if leaf == end_leaf {
            end.byte_offset.min(text_len)
        } else {
            text_len
        };
        if s < e {
            ranges.push((bp, s, e));
        }
    }
    ranges
}

/// Returns the word boundary around `byte_offset` in `text` as
/// `(word_start_byte, word_end_byte)`.
///
/// A "word character" is alphanumeric or underscore.  If the cursor is
/// on whitespace or punctuation, `word_start == word_end` (empty word).
fn word_bounds_at(text: &str, byte_offset: usize) -> (usize, usize) {
    let clamped = byte_offset.min(text.len());
    let before = &text[..clamped];

    // Walk backward to find the last non-word character, then word starts one
    // character after it.
    let word_start = match before
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphanumeric() && *c != '_')
    {
        Some((i, c)) => i + c.len_utf8(),
        None => 0,
    };

    // Walk forward from clamped to find the first non-word character.
    let after = &text[clamped..];
    let word_end = clamped
        + match after
            .char_indices()
            .find(|(_, c)| !c.is_alphanumeric() && *c != '_')
        {
            Some((i, _)) => i,
            None => after.len(),
        };

    (word_start, word_end)
}
