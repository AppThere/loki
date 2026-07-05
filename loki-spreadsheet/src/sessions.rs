// SPDX-License-Identifier: Apache-2.0

//! In-memory editing sessions for inactive workbook tabs.
//!
//! When the user switches away from a workbook tab, `EditorInner` stashes the
//! live CRDT and grid state here instead of discarding it; switching back
//! restores the session so unsaved edits survive tab switches (plan 4b.6,
//! mirrors `loki-text/src/sessions.rs`). Closing a tab drops its session (see
//! `routes/shell.rs`).
//!
//! The map is provided as `Signal<DocSessions>` in Dioxus context at the
//! [`crate::app::App`] root. Sessions exist only for *inactive* tabs — the
//! active tab's state lives in the editor signals. The dirty flag needs no
//! stashing: it lives on the tab entry itself (`OpenTab::is_dirty`).

use std::collections::HashMap;

use loki_sheet_model::Workbook;

/// Live editing state for one open-but-inactive workbook.
pub struct DocSession {
    /// The live CRDT — holds all unsaved edits.
    pub loro_doc: loro::LoroDoc,
    /// Undo history paired with `loro_doc`.
    pub undo_manager: Option<loro::UndoManager>,
    /// Post-mutation workbook snapshot the grid renders from.
    pub workbook: Workbook,
    /// Ribbon/keyboard undo/redo state at stash time.
    pub can_undo: bool,
    /// Ribbon/keyboard undo/redo state at stash time.
    pub can_redo: bool,
    /// The selected cell at stash time (restored so the user resumes in place).
    pub selected_cell: Option<(usize, usize)>,
}

/// Open-but-inactive workbook sessions, keyed by the serialised file token.
pub type DocSessions = HashMap<String, DocSession>;
