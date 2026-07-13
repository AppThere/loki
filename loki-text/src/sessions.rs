// SPDX-License-Identifier: Apache-2.0

//! In-memory editing sessions for inactive document tabs.
//!
//! When the user switches away from a document tab, `EditorInner` stashes the
//! live CRDT and model state here instead of discarding it; switching back
//! restores the session so unsaved edits survive tab switches.  Closing a tab
//! drops its session (see `routes/shell.rs`).
//!
//! The **layout is deliberately not stashed** (memory-audit F3 / plan 6.1):
//! a preserved `PaginatedLayout` pins Parley layouts + byte maps for every
//! paragraph (~MBs per inactive tab). Restore recomputes it from the stashed
//! [`Document`] on the open path's worker thread instead — O(1) inactive-tab
//! memory, matching the model-only pattern of the spreadsheet/presentation
//! session maps.
//!
//! The map is provided as `Signal<DocSessions>` in Dioxus context at the
//! [`crate::app::App`] root.  Sessions exist only for *inactive* tabs — the
//! active tab's state lives in the editor signals.

use std::collections::HashMap;
use std::sync::Arc;

use loki_doc_model::document::Document;

use crate::editing::cursor::CursorState;

/// Live editing state for one open-but-inactive document.
///
/// Deliberately excludes `shared_font_resources`: the Parley font context is
/// per-editor (one `EditorInner` instance survives all tab switches) and is
/// expensive to rebuild, so it stays in the live `DocumentState`.
pub struct DocSession {
    /// The live CRDT — holds all unsaved edits.
    pub loro_doc: loro::LoroDoc,
    /// Undo history paired with `loro_doc`.
    pub undo_manager: Option<loro::UndoManager>,
    /// Post-mutation document snapshot — the relayout-on-restore input.
    pub document: Option<Arc<Document>>,
    /// Mutation counter from `DocumentState`.
    pub generation: u64,
    /// Cursor/selection state, including the mirrored document generation.
    pub cursor: CursorState,
    /// Document generation considered clean (matches the on-disk file).
    pub baseline_gen: u64,
    /// Undo-stack clean-checkpoint tracker paired with `undo_manager`.
    pub saved_state: crate::editing::saved_state::SavedStateHandle,
    /// Ribbon undo/redo button state at stash time.
    pub can_undo: bool,
    /// Ribbon undo/redo button state at stash time.
    pub can_redo: bool,
}

/// Open-but-inactive document sessions, keyed by the serialised file token.
pub type DocSessions = HashMap<String, DocSession>;
