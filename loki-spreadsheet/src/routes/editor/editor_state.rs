// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Per-document editor signals and shared state initialisation for loki-spreadsheet.

use dioxus::prelude::*;

/// All per-document signals for the spreadsheet editor, grouped for ergonomic initialisation.
pub(super) struct EditorState {
    pub workbook_snap: Signal<loki_sheet_model::Workbook>,
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    pub can_undo: Signal<bool>,
    pub can_redo: Signal<bool>,
    pub selected_cell: Signal<Option<(usize, usize)>>,
    pub editing_cell: Signal<Option<(usize, usize)>>,
}

/// Initialises and returns all per-document editing signals.
pub(super) fn use_editor_state() -> EditorState {
    EditorState {
        workbook_snap: use_signal(loki_sheet_model::Workbook::new),
        loro_doc: use_signal(|| None),
        undo_manager: use_signal(|| None),
        can_undo: use_signal(|| false),
        can_redo: use_signal(|| false),
        selected_cell: use_signal(|| Some((0, 0))),
        editing_cell: use_signal(|| None),
    }
}
