// SPDX-License-Identifier: Apache-2.0

//! How much text the selection covers, in characters.
//!
//! Split from `body` for the file ceiling; it is the one piece of that module
//! that reads the document rather than rendering it.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use super::super::dialog_walk::{block_at_path, block_text};
use super::super::editor_format_range::resolve_format_ranges;
use super::super::editor_insert_sync::InsertLinkSync;
use crate::editing::state::DocumentState;

/// How many characters the selection covers, across every block it spans.
///
/// **Characters, not bytes.** The cursor's offsets are byte offsets into each
/// block's text, so subtracting them reported 6 for the five-character "héllo"
/// and, for a selection spanning three paragraphs, a difference between two
/// unrelated offsets — a number with no relation to what was selected.
///
/// The ranges come from `resolve_format_ranges`, the same per-block split that
/// `apply_marks` writes through, so the count describes exactly the text the
/// dialog will act on.
#[must_use]
pub(super) fn selection_len(doc_state: &Arc<Mutex<DocumentState>>, sync: &InsertLinkSync) -> usize {
    let cursor = sync.cursor_state.read().clone();
    let guard = sync.loro_doc.read();
    let Some(ldoc) = guard.as_ref() else {
        return 0;
    };
    let ranges = resolve_format_ranges(ldoc, &cursor);
    let Ok(state) = doc_state.lock() else {
        return 0;
    };
    let Some(doc) = state.document.as_ref() else {
        return 0;
    };

    ranges
        .iter()
        .map(|(path, start, end)| {
            let text = block_at_path(doc, path).map(block_text).unwrap_or_default();
            let (start, end) = (*start.min(end), *start.max(end));
            let end = end.min(text.len());
            let start = start.min(end);
            // Byte offsets can only land on a boundary if the range came from
            // the model, but a stale cursor is cheap to guard against.
            text.get(start..end).map(|s| s.chars().count()).unwrap_or(0)
        })
        .sum()
}
