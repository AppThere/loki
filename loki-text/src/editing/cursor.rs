// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Cursor and selection state for the Loki document editor.

use unicode_segmentation::UnicodeSegmentation;

/// A document position identified by page index, paragraph index within the
/// page, and byte offset within the paragraph.
///
/// All three fields are zero-based.  `paragraph_index` is an index into
/// [`loki_layout::PageEditingData::paragraph_layouts`] for the given page.
/// `byte_offset` is a byte offset into the paragraph's flattened text content.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentPosition {
    /// Zero-based index of the page that contains this position.
    pub page_index: usize,
    /// Zero-based index of the paragraph on that page (into
    /// `PageEditingData::paragraph_layouts`).
    pub paragraph_index: usize,
    /// Byte offset into the paragraph's flattened text content.
    pub byte_offset: usize,
}

/// The current cursor and selection state for the editor.
///
/// Anchor and focus are both `None` before the user places a cursor
/// for the first time in editing mode.
#[derive(Debug, Clone)]
pub struct CursorState {
    /// Stable Loro cursor anchored to a Fugue element ID.
    ///
    /// Survives concurrent edits and remote operations. `None` before the
    /// first click in editing mode, or when [`derive_loro_cursor`] cannot
    /// resolve the position (e.g. cross-page layout/block mapping is not yet
    /// implemented).
    ///
    /// [`derive_loro_cursor`]: loki_doc_model::loro_bridge::derive_loro_cursor
    pub loro_cursor: Option<loro::cursor::Cursor>,
    /// Anchor end of the selection — the point where the drag started.
    ///
    /// Equal to [`focus`] when no range selection is active (point cursor).
    ///
    /// [`focus`]: CursorState::focus
    pub anchor: Option<DocumentPosition>,
    /// Focus end of the selection — the current (moving) cursor position.
    pub focus: Option<DocumentPosition>,
}

impl PartialEq for CursorState {
    fn eq(&self, other: &Self) -> bool {
        // We primarily care about the visual positions (anchor/focus) for
        // re-rendering decisions. loro_cursor is a stable pointer that
        // usually moves in sync with focus.
        self.anchor == other.anchor && self.focus == other.focus
    }
}

impl CursorState {
    /// Returns an empty cursor state (no cursor placed yet).
    pub fn new() -> Self {
        Self { loro_cursor: None, anchor: None, focus: None }
    }

    /// Returns `true` when a range selection exists (anchor differs from focus).
    pub fn has_selection(&self) -> bool {
        self.anchor.is_some() && self.focus.is_some() && self.anchor != self.focus
    }
}

impl Default for CursorState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Grapheme boundary helpers ─────────────────────────────────────────────────

/// Returns the byte offset of the previous grapheme cluster boundary strictly
/// before `offset` in `text`.
///
/// Returns `0` when `offset` is already at the start of the string or when
/// `text` is empty.  Handles multi-byte characters and emoji clusters
/// correctly via [`unicode_segmentation`].
pub fn prev_grapheme_boundary(text: &str, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }
    // Walk grapheme clusters from the start and keep track of the most recent
    // byte boundary that is strictly before `offset`.
    let mut prev = 0usize;
    for (idx, _grapheme) in text.grapheme_indices(true) {
        if idx >= offset {
            break;
        }
        prev = idx;
    }
    prev
}

/// Returns the byte offset of the next grapheme cluster boundary strictly
/// after `offset` in `text`.
///
/// Returns `text.len()` when `offset` is already at or past the end of the
/// string.  Handles multi-byte characters and emoji clusters correctly via
/// [`unicode_segmentation`].
pub fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    // Find the first grapheme cluster end that is strictly after `offset`.
    for (idx, grapheme) in text.grapheme_indices(true) {
        let end = idx + grapheme.len();
        if end > offset {
            return end;
        }
    }
    text.len()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prev_grapheme_in_ascii_mid() {
        // "hello": h(0) e(1) l(2) l(3) o(4)
        assert_eq!(prev_grapheme_boundary("hello", 3), 2);
    }

    #[test]
    fn prev_grapheme_at_start() {
        assert_eq!(prev_grapheme_boundary("hello", 0), 0);
    }

    #[test]
    fn next_grapheme_in_ascii_mid() {
        assert_eq!(next_grapheme_boundary("hello", 2), 3);
    }

    #[test]
    fn next_grapheme_at_end() {
        assert_eq!(next_grapheme_boundary("hello", 5), 5);
    }

    #[test]
    fn prev_grapheme_multibyte() {
        // "héllo": h(0) é(1..3) l(3) l(4) o(5)
        // é is U+00E9, encoded as 0xC3 0xA9 — 2 bytes.
        // byte 3 is the start of 'l'; prev boundary should be 1 (start of é).
        let s = "héllo";
        assert_eq!(prev_grapheme_boundary(s, 3), 1);
    }

    #[test]
    fn next_grapheme_emoji() {
        // "a😀b": a(0) 😀(1..5) b(5)
        // 😀 is U+1F600, encoded as 4 bytes.
        // next boundary after offset 1 should be 5 (end of emoji / start of 'b').
        let s = "a\u{1F600}b";
        assert_eq!(next_grapheme_boundary(s, 1), 5);
    }
}
