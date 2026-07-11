// SPDX-License-Identifier: Apache-2.0

//! Text (font) colour for the ribbon.
//!
//! Colour is a character mark ([`MARK_COLOR`]); for an RGB colour the stored
//! value is simply its `#RRGGBB` hex string (the same form the bridge's colour
//! codec writes for `DocumentColor::Rgb`), so applying a preset swatch needs no
//! extra encoding. Like the other character formats it goes through the
//! path-aware [`mark_text_at`] + [`resolve_format_ranges`] path, so it works in
//! table cells and across a multi-paragraph selection.

use loki_doc_model::loro_schema::MARK_COLOR;
use loki_doc_model::{MutationError, get_mark_at_path, mark_text_at};
use loro::{LoroDoc, LoroValue};

use super::editor_format_range::resolve_format_ranges;
use crate::editing::cursor::CursorState;

/// Applies `hex` (`Some("#RRGGBB")`) as the text colour across the selection, or
/// removes the direct colour mark (`None` → "Automatic", reverting to the
/// paragraph/character style's colour).
pub(super) fn apply_text_color(
    loro: &LoroDoc,
    cursor: &CursorState,
    hex: Option<&str>,
) -> Result<(), MutationError> {
    let value = match hex {
        Some(h) => LoroValue::from(h.to_string()),
        None => LoroValue::Null,
    };
    for (path, start, end) in &resolve_format_ranges(loro, cursor) {
        mark_text_at(loro, path, *start, *end, MARK_COLOR, value.clone())?;
    }
    Ok(())
}

/// The direct text-colour hex at the caret's first resolved range, or `None`
/// when the range has no explicit colour (its colour comes from a style). Drives
/// which swatch shows as active.
pub(super) fn current_text_color(loro: &LoroDoc, cursor: &CursorState) -> Option<String> {
    let ranges = resolve_format_ranges(loro, cursor);
    let (path, start, _) = ranges.first()?;
    match get_mark_at_path(loro, path, *start, MARK_COLOR) {
        Ok(Some(LoroValue::String(s))) => Some(s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
#[path = "editor_text_color_tests.rs"]
mod tests;
