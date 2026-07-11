// SPDX-License-Identifier: Apache-2.0

//! Text highlight colour for the ribbon.
//!
//! Highlight is a character mark ([`MARK_HIGHLIGHT_COLOR`]) whose value is a
//! `HighlightColor` **variant name** (`"Yellow"`, `"Green"`, …, or `"None"`) —
//! the same string `decode_highlight_color` reads back. Like the other
//! character formats it applies through the path-aware [`mark_text_at`] +
//! [`resolve_format_ranges`] path, so it works in table cells and across a
//! multi-paragraph selection.

use loki_doc_model::loro_schema::MARK_HIGHLIGHT_COLOR;
use loki_doc_model::{MutationError, get_mark_at_path, mark_text_at};
use loro::{LoroDoc, LoroValue};

use super::editor_format_range::resolve_format_ranges;
use crate::editing::cursor::CursorState;

/// Applies the highlight `name` (`Some("Yellow")`, …) across the selection, or
/// removes the direct highlight mark (`None` → no highlight).
pub(super) fn apply_highlight(
    loro: &LoroDoc,
    cursor: &CursorState,
    name: Option<&str>,
) -> Result<(), MutationError> {
    let value = match name {
        Some(n) => LoroValue::from(n.to_string()),
        None => LoroValue::Null,
    };
    for (path, start, end) in &resolve_format_ranges(loro, cursor) {
        mark_text_at(
            loro,
            path,
            *start,
            *end,
            MARK_HIGHLIGHT_COLOR,
            value.clone(),
        )?;
    }
    Ok(())
}

/// The direct highlight variant name at the caret's first resolved range, or
/// `None` when there is no explicit highlight. Drives the active swatch.
pub(super) fn current_highlight(loro: &LoroDoc, cursor: &CursorState) -> Option<String> {
    let ranges = resolve_format_ranges(loro, cursor);
    let (path, start, _) = ranges.first()?;
    match get_mark_at_path(loro, path, *start, MARK_HIGHLIGHT_COLOR) {
        Ok(Some(LoroValue::String(s))) => Some(s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "editor_highlight_color_tests.rs"]
mod tests;
