// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Cursor and selection state for the Loki document editor.

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
