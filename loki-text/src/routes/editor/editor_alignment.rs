// SPDX-License-Identifier: Apache-2.0

//! Paragraph-alignment actions for the ribbon.
//!
//! Reads/sets alignment on the caret's paragraph(s) using the path-aware
//! `loki_doc_model` mutations, so alignment works in table cells and note bodies
//! as well as top-level paragraphs. Range resolution reuses
//! [`resolve_format_ranges`](super::editor_format_range::resolve_format_ranges),
//! the same per-paragraph mapping the inline-format toggles use.

use loki_doc_model::{MutationError, get_block_alignment_at, set_block_alignment_at};
use loro::LoroDoc;

use super::editor_format_range::resolve_format_ranges;
use crate::editing::cursor::CursorState;

/// The alignment of the caret's paragraph — the first resolved range's block
/// (`"Left"` when there is no cursor). Drives the ribbon buttons' active state.
pub(super) fn current_alignment(loro: &LoroDoc, cursor: &CursorState) -> String {
    match resolve_format_ranges(loro, cursor).first() {
        Some((path, _, _)) => get_block_alignment_at(loro, path),
        None => "Left".to_string(),
    }
}

/// Sets `alignment` on every paragraph in the selection (or the caret's
/// paragraph for a point cursor). One `BlockPath` per paragraph, so a
/// multi-paragraph selection is aligned uniformly.
pub(super) fn apply_alignment(
    loro: &LoroDoc,
    cursor: &CursorState,
    alignment: &str,
) -> Result<(), MutationError> {
    for (path, _, _) in &resolve_format_ranges(loro, cursor) {
        set_block_alignment_at(loro, path, alignment)?;
    }
    Ok(())
}
