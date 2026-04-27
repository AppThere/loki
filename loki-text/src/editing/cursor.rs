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

// ── Grapheme boundary navigation ──────────────────────────────────────────────

/// Return the byte offset of the start of the grapheme cluster immediately
/// before `byte_offset` in `text`.
///
/// Returns `0` when `byte_offset` is already at the start of the string.  If
/// `byte_offset` falls inside a multi-byte grapheme, the start of that grapheme
/// is returned.
pub fn prev_grapheme_boundary(text: &str, byte_offset: usize) -> usize {
    if byte_offset == 0 {
        return 0;
    }
    let mut prev_start = 0;
    for g in text.graphemes(true) {
        let next = prev_start + g.len();
        if next >= byte_offset {
            return prev_start;
        }
        prev_start = next;
    }
    prev_start
}

/// Return the byte offset of the start of the grapheme cluster immediately
/// after `byte_offset` in `text`.
///
/// Returns `text.len()` when `byte_offset` is at or beyond the end of the
/// string.
pub fn next_grapheme_boundary(text: &str, byte_offset: usize) -> usize {
    let mut offset = 0;
    for g in text.graphemes(true) {
        offset += g.len();
        if offset > byte_offset {
            return offset;
        }
    }
    text.len()
}

// ── Grapheme tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod grapheme_tests {
    use super::*;

    #[test]
    fn prev_at_start_returns_zero() {
        assert_eq!(prev_grapheme_boundary("Hello", 0), 0);
    }

    #[test]
    fn prev_grapheme_ascii() {
        assert_eq!(prev_grapheme_boundary("Hello", 1), 0);
        assert_eq!(prev_grapheme_boundary("Hello", 3), 2);
        assert_eq!(prev_grapheme_boundary("Hello", 5), 4);
    }

    #[test]
    fn prev_grapheme_multibyte() {
        // "é" = U+00E9 = 2 bytes (0xC3 0xA9).
        // "Héllo": H=0..1, é=1..3, l=3..4, l=4..5, o=5..6
        assert_eq!(prev_grapheme_boundary("H\u{00E9}llo", 3), 1); // before "é" end
        assert_eq!(prev_grapheme_boundary("H\u{00E9}llo", 1), 0); // before "H" end
    }

    #[test]
    fn next_grapheme_ascii() {
        assert_eq!(next_grapheme_boundary("Hello", 0), 1);
        assert_eq!(next_grapheme_boundary("Hello", 3), 4);
        assert_eq!(next_grapheme_boundary("Hello", 4), 5);
    }

    #[test]
    fn next_grapheme_at_end_returns_len() {
        assert_eq!(next_grapheme_boundary("Hello", 5), 5);
        assert_eq!(next_grapheme_boundary("", 0), 0);
    }

    #[test]
    fn next_grapheme_multibyte() {
        // "é" = 2 bytes starting at offset 1.
        assert_eq!(next_grapheme_boundary("H\u{00E9}llo", 1), 3); // skip "é"
        assert_eq!(next_grapheme_boundary("H\u{00E9}llo", 0), 1); // skip "H"
    }
}
