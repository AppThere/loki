// SPDX-License-Identifier: Apache-2.0

//! Reading and applying the selection's **character style** reference.
//!
//! A character style is a *style level*, not direct formatting: it sits under
//! the direct marks and above the paragraph style (design note 09), so its key
//! is deliberately absent from [`super::marks::OWNED_MARKS`] — "Clear direct
//! formatting" must leave the style reference in place. It still travels as a
//! Loro mark ([`MARK_CHAR_STYLE_ID`]) because a style reference on a run of
//! text is span-scoped data; the bridge round-trips it and the layout resolver
//! walks the referenced style's chain (`resolve_char_span`).

use loki_doc_model::MARK_CHAR_STYLE_ID;
use loki_doc_model::{MutationError, get_mark_at_path, mark_text_at};
use loro::{LoroDoc, LoroValue};

use super::super::editor_format_range::resolve_format_ranges;
use crate::editing::cursor::CursorState;

/// The character-style id at the start of the selection — the same
/// head-of-range convention [`super::marks::read_marks`] documents.
#[must_use]
pub(super) fn read_char_style(loro: &LoroDoc, cursor: &CursorState) -> Option<String> {
    let ranges = resolve_format_ranges(loro, cursor);
    let (path, start, _) = ranges.first()?;
    match get_mark_at_path(loro, path, *start, MARK_CHAR_STYLE_ID)
        .ok()
        .flatten()
    {
        Some(LoroValue::String(s)) => Some(s.to_string()),
        _ => None,
    }
}

/// Writes the staged character style over the selection when it changed.
/// `None` removes the reference — the run falls back to the paragraph level.
pub(super) fn apply_char_style(
    loro: &LoroDoc,
    cursor: &CursorState,
    before: &Option<String>,
    next: &Option<String>,
) -> Result<(), MutationError> {
    if before == next {
        return Ok(());
    }
    let value = next.clone().map_or(LoroValue::Null, LoroValue::from);
    for (path, start, end) in resolve_format_ranges(loro, cursor) {
        mark_text_at(loro, &path, start, end, MARK_CHAR_STYLE_ID, value.clone())?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "char_style_tests.rs"]
mod tests;
