// SPDX-License-Identifier: Apache-2.0

//! The write-side signal bundle the insert-and-format dialogs share.
//!
//! This was `editor_insert_panel`, the Insert tab's one-field hyperlink panel.
//! The panel was retired when the Insert link dialog took over that button; the
//! bundle it defined outlived it, because the link, table and span dialogs all
//! need exactly these five handles to persist through Loro and refresh the
//! undo/dirty state.

use dioxus::prelude::*;

use crate::editing::cursor::CursorState;

/// Signals a dialog needs to persist a mutation through Loro and refresh the
/// undo/redo state. Grouped to keep the function signatures manageable.
///
/// `PartialEq` compares the [`Signal`] handles rather than the values behind
/// them — "the same signals", which is what a props comparison wants (the same
/// trade `SpellSync` makes). The link, table and span dialogs are components and
/// need their props to compare.
#[derive(Clone, Copy, PartialEq)]
pub(in crate::routes::editor) struct InsertLinkSync {
    /// The document's Loro CRDT handle.
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    /// Cursor state (mirrors the document generation for dirty tracking).
    pub cursor_state: Signal<CursorState>,
    /// Undo manager, refreshed after the mutation.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether undo is available.
    pub can_undo: Signal<bool>,
    /// Whether redo is available.
    pub can_redo: Signal<bool>,
}
