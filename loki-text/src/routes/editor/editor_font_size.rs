// SPDX-License-Identifier: Apache-2.0

//! Font-size grow/shrink for the ribbon.
//!
//! Font size is a character mark ([`MARK_FONT_SIZE_PT`], a point value), so it
//! applies to the caret's paragraph(s) through the same path-aware
//! [`mark_text_at`] + [`resolve_format_ranges`] path the inline toggles use —
//! it works in table cells and across a multi-paragraph selection.
//!
//! Grow/shrink step through a fixed ladder of common sizes. The current size is
//! read from the selection's **direct** size mark; a range with no explicit size
//! (its size comes from a style) steps from [`DEFAULT_FONT_SIZE_PT`].

use loki_doc_model::loro_schema::MARK_FONT_SIZE_PT;
use loki_doc_model::{MutationError, get_mark_at_path, mark_text_at};
use loro::{LoroDoc, LoroValue};

use super::editor_format_range::resolve_format_ranges;
use crate::editing::cursor::CursorState;

/// Common point sizes the grow/shrink buttons step through (Word's ladder).
const SIZE_LADDER: &[f64] = &[
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 40.0, 44.0,
    48.0, 54.0, 60.0, 66.0, 72.0, 80.0, 88.0, 96.0,
];

/// Fallback size when the selection carries no direct size mark.
const DEFAULT_FONT_SIZE_PT: f64 = 11.0;

/// The next ladder size strictly greater than `size` (clamped to the top).
fn grow(size: f64) -> f64 {
    SIZE_LADDER
        .iter()
        .copied()
        .find(|&s| s > size + f64::EPSILON)
        .unwrap_or(96.0)
}

/// The previous ladder size strictly less than `size` (clamped to the bottom).
fn shrink(size: f64) -> f64 {
    SIZE_LADDER
        .iter()
        .rev()
        .copied()
        .find(|&s| s < size - f64::EPSILON)
        .unwrap_or(8.0)
}

/// The direct font size (pt) at the caret's first resolved range, or
/// [`DEFAULT_FONT_SIZE_PT`] when it has no explicit size mark.
fn current_font_size(loro: &LoroDoc, cursor: &CursorState) -> f64 {
    let ranges = resolve_format_ranges(loro, cursor);
    let Some((path, start, _)) = ranges.first() else {
        return DEFAULT_FONT_SIZE_PT;
    };
    match get_mark_at_path(loro, path, *start, MARK_FONT_SIZE_PT) {
        Ok(Some(LoroValue::Double(v))) => v,
        _ => DEFAULT_FONT_SIZE_PT,
    }
}

/// Applies `size_pt` across every resolved range (the selection, or the word at
/// a point cursor).
fn apply_font_size(
    loro: &LoroDoc,
    cursor: &CursorState,
    size_pt: f64,
) -> Result<(), MutationError> {
    for (path, start, end) in &resolve_format_ranges(loro, cursor) {
        mark_text_at(
            loro,
            path,
            *start,
            *end,
            MARK_FONT_SIZE_PT,
            LoroValue::Double(size_pt),
        )?;
    }
    Ok(())
}

/// Grows (`grow_it = true`) or shrinks the selection's font size by one ladder
/// step from its current size.
pub(super) fn adjust_font_size(
    loro: &LoroDoc,
    cursor: &CursorState,
    grow_it: bool,
) -> Result<(), MutationError> {
    let current = current_font_size(loro, cursor);
    let next = if grow_it {
        grow(current)
    } else {
        shrink(current)
    };
    apply_font_size(loro, cursor, next)
}

#[cfg(test)]
#[path = "editor_font_size_tests.rs"]
mod tests;
