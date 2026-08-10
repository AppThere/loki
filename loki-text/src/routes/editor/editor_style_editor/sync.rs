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
/// `editor_insert_sync::InsertLinkSync`).
///
/// `PartialEq` compares the [`Signal`] handles, not the values behind them, so
/// it means "the same signals" rather than "the same document" — which is what a
/// props comparison wants (the same trade `SpellSync` makes). The paragraph
/// style dialog is a real component and needs its props to compare.
#[derive(Clone, Copy, PartialEq)]
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
    /// Bumped whenever an **app-scoped** setting is written (T6.3/T6.4).
    ///
    /// The panel reads those settings from disk on each render, so it needs a
    /// signal to re-render *on* — a settings file is not reactive state. An
    /// explicit counter rather than re-setting an unrelated signal to its own
    /// value: that works only because `Signal::set` does not compare, and reads
    /// at the call site as a line that does nothing.
    pub settings_generation: Signal<u64>,
}
