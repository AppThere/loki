// SPDX-License-Identifier: Apache-2.0

//! The style editor's write-side signal bundle, split from [`super`] so that
//! module stays under the 300-line ceiling.
//!
//! Every control in the panel that mutates the document needs the same five
//! handles plus the status sink; passing them as one `Copy` struct is what keeps
//! `style_editor_panel` and each family's form from growing a six-argument tail.

use dioxus::prelude::*;

use super::super::editor_state::SaveStatus;
use crate::editing::cursor::CursorState;

/// Signals the style editor needs to persist edits through Loro and refresh the
/// undo/redo state. Grouped to keep the function signature manageable (mirrors
/// `editor_metadata_panel::MetaPanelSync`).
#[derive(Clone, Copy)]
pub(in crate::routes::editor) struct StyleEditorSync {
    /// The document's Loro CRDT handle.
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    /// Cursor state (mirrors the document generation for dirty tracking).
    pub cursor_state: Signal<CursorState>,
    /// Undo manager, refreshed after the style mutation.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether undo is available.
    pub can_undo: Signal<bool>,
    /// Whether redo is available.
    pub can_redo: Signal<bool>,
    /// Status-banner sink for feedback (e.g. a rejected cyclic re-parent).
    pub save_message: Signal<Option<SaveStatus>>,
}
