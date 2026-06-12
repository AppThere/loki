// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cursor and selection rendering types.

use loki_layout::CursorRect;

// Selection-handle dimensions (in layout points).
pub(super) const HANDLE_STEM_HEIGHT: f32 = 24.0;
pub(super) const HANDLE_CIRCLE_RADIUS: f32 = 8.0;

/// Whether a selection handle is at the anchor (start) or focus (end) of the
/// selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionHandleKind {
    /// Anchor handle — shown at the start of the selection.
    Anchor,
    /// Focus handle — shown at the end of the selection.
    Focus,
}

/// A teardrop-shaped selection handle rendered at the edge of a mobile selection.
///
/// Handles are only shown on iOS and Android (controlled by `#[cfg(target_os)]`
/// in `editor.rs`). On desktop the cursor and selection highlights are
/// sufficient — drag handles would look out of place.
#[derive(Debug, Clone)]
pub struct SelectionHandle {
    /// X position of the handle tip in page-content-area coordinates (points).
    pub tip_x: f32,
    /// Y position of the handle tip in page-content-area coordinates (points).
    pub tip_y: f32,
    /// Whether this is the anchor (start) or focus (end) handle.
    pub kind: SelectionHandleKind,
}

/// A highlight rectangle for a selection range, in paragraph-local coordinates
/// (points).
#[derive(Debug, Clone, Copy)]
pub struct SelectionRect {
    /// X position of the rectangle's left edge in paragraph-local coordinates.
    pub x: f32,
    /// Y position of the rectangle's top edge in paragraph-local coordinates.
    pub y: f32,
    /// Width of the rectangle in points.
    pub width: f32,
    /// Height of the rectangle in points.
    pub height: f32,
}

/// Cursor and selection highlight data for a single paragraph on one page.
///
/// All rects are in paragraph-local coordinates (points, origin at the
/// paragraph's `(0, 0)` top-left). The painter applies the paragraph origin
/// and page content-area offset at render time.
#[derive(Debug, Clone)]
pub struct CursorPaint {
    /// Visual cursor rect, or `None` when the cursor has no position in this
    /// paragraph.
    pub cursor_rect: Option<CursorRect>,
    /// Zero or more selection highlight rects.  Empty when no range selection
    /// is active.
    pub selection_rects: Vec<SelectionRect>,
    /// Selection handles for mobile (iOS/Android) long-press word selection.
    ///
    /// Populated only when a range selection is active on a touch device.
    /// Empty on desktop — handles are guarded by `#[cfg(target_os)]` in the
    /// caller.
    pub selection_handles: Vec<SelectionHandle>,
    /// Global index of the paragraph block that this data belongs to.
    /// Used by the painter to look up the paragraph's page-local origin.
    pub paragraph_index: usize,
}
