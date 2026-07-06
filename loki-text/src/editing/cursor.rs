// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cursor and selection state for the Loki document editor.

use loki_doc_model::{BlockPath, PathStep};
use unicode_segmentation::UnicodeSegmentation;

/// A document position identified by page index, paragraph index within the
/// page, and byte offset within the paragraph.
///
/// All three index fields are zero-based.  `paragraph_index` is an index into
/// [`loki_layout::PageEditingData::paragraph_layouts`] for the given page (which
/// is the document-global block index of the paragraph, or its **root** block
/// when nested).  `byte_offset` is a byte offset into the paragraph's flattened
/// text content.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocumentPosition {
    /// Zero-based index of the page that contains this position.
    pub page_index: usize,
    /// Zero-based index of the paragraph on that page (into
    /// `PageEditingData::paragraph_layouts`).
    pub paragraph_index: usize,
    /// Byte offset into the paragraph's flattened text content.
    pub byte_offset: usize,
    /// Descent into a nested container (table cell / note body). Empty for an
    /// ordinary top-level paragraph. With `paragraph_index` as the root this
    /// forms the [`BlockPath`] the editor uses to address the paragraph.
    pub path: Vec<PathStep>,
}

impl DocumentPosition {
    /// A top-level position (no nesting) — the common case.
    #[must_use]
    pub fn top_level(page_index: usize, paragraph_index: usize, byte_offset: usize) -> Self {
        Self {
            page_index,
            paragraph_index,
            byte_offset,
            path: Vec::new(),
        }
    }

    /// Whether two positions address the same caret location, ignoring
    /// `page_index`.
    ///
    /// `page_index` is a display-only artifact: a paragraph flowing across a
    /// page break has an entry on every page it touches, so the same logical
    /// caret `(paragraph_index, byte_offset, path)` can carry different
    /// `page_index` values depending on which fragment produced it. Selection
    /// logic must compare on the logical caret so a "phantom" zero-width
    /// selection differing only in `page_index` is not treated as a range.
    #[must_use]
    pub fn same_caret(&self, other: &DocumentPosition) -> bool {
        self.paragraph_index == other.paragraph_index
            && self.byte_offset == other.byte_offset
            && self.path == other.path
    }

    /// The [`BlockPath`] addressing this position's paragraph for mutation.
    #[must_use]
    pub fn block_path(&self) -> BlockPath {
        BlockPath {
            root: self.paragraph_index,
            steps: self.path.clone(),
        }
    }

    /// The position of a sibling block within the **same container**, shifted by
    /// `delta` blocks, at `byte_offset`.
    ///
    /// For a top-level position this shifts `paragraph_index`; for a nested
    /// position (inside a table cell / note body) it shifts the leaf
    /// [`PathStep`]'s block index, leaving the root `paragraph_index` untouched.
    /// Used to place the cursor after a paragraph split (`delta = 1`, offset 0)
    /// or merge (`delta = -1`, offset = the join point).
    #[must_use]
    pub fn sibling_block(&self, delta: isize, byte_offset: usize) -> Self {
        let mut pos = self.clone();
        pos.byte_offset = byte_offset;
        match pos.path.last_mut() {
            Some(PathStep::Cell { block, .. } | PathStep::Note { block, .. }) => {
                *block = block.saturating_add_signed(delta);
            }
            None => {
                pos.paragraph_index = pos.paragraph_index.saturating_add_signed(delta);
            }
        }
        pos
    }
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
    /// Mirrors `DocumentState::generation` — incremented after every document
    /// mutation so the `data-cursor` canvas attribute changes even when the
    /// cursor position is unchanged (e.g. after a formatting toggle).  Without
    /// this, Blitz would not mark the canvas node dirty and `render()` would
    /// not be called after formatting.
    pub document_generation: u64,
}

impl PartialEq for CursorState {
    fn eq(&self, other: &Self) -> bool {
        // Include document_generation so formatting changes (which do not move
        // the cursor) still cause PageCanvas to re-render and update data-cursor.
        self.anchor == other.anchor
            && self.focus == other.focus
            && self.document_generation == other.document_generation
    }
}

impl CursorState {
    /// Returns an empty cursor state (no cursor placed yet).
    pub fn new() -> Self {
        Self {
            loro_cursor: None,
            anchor: None,
            focus: None,
            document_generation: 0,
        }
    }

    /// Returns `true` when a range selection exists — the anchor and focus
    /// address different caret locations.
    ///
    /// Compares on the logical caret (via [`DocumentPosition::same_caret`]), so
    /// two positions that differ only in `page_index` (a page-spanning
    /// paragraph fragment) do not register as a phantom zero-width selection
    /// that would swallow the next Backspace/Delete.
    pub fn has_selection(&self) -> bool {
        match (self.anchor.as_ref(), self.focus.as_ref()) {
            (Some(a), Some(f)) => !a.same_caret(f),
            _ => false,
        }
    }

    /// The [`BlockPath`] of the focus position, if a cursor is placed.
    ///
    /// `BlockPath::block(i)` for a top-level cursor; a nested path when the
    /// focus is inside a table cell / note body.
    pub fn block_path(&self) -> Option<BlockPath> {
        self.focus.as_ref().map(DocumentPosition::block_path)
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
#[path = "cursor_tests.rs"]
mod tests;
