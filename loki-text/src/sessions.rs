// SPDX-License-Identifier: Apache-2.0

//! In-memory editing sessions for inactive document tabs.
//!
//! When the user switches away from a document tab, `EditorInner` stashes the
//! live CRDT and layout state here instead of discarding it; switching back
//! restores the session so unsaved edits survive tab switches.  Closing a tab
//! drops its session (see `routes/shell.rs`).
//!
//! The map is provided as `Signal<DocSessions>` in Dioxus context at the
//! [`crate::app::App`] root.  Sessions exist only for *inactive* tabs — the
//! active tab's state lives in the editor signals.

use std::collections::HashMap;
use std::sync::Arc;

use loki_doc_model::document::Document;
use loki_layout::PaginatedLayout;

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
    /// Post-mutation document snapshot (shared with the renderer).
    pub document: Option<Arc<Document>>,
    /// Mutation counter from `DocumentState`.
    pub generation: u64,
    /// Page count from the stashed layout.
    pub page_count: usize,
    /// Paginated layout for hit-testing, restored as-is.
    pub paginated_layout: Option<Arc<PaginatedLayout>>,
    /// Page dimensions in CSS px.
    pub page_width_px: f32,
    /// Page dimensions in CSS px.
    pub page_height_px: f32,
    /// Cursor/selection state, including the mirrored document generation.
    pub cursor: CursorState,
    /// Document generation considered clean (matches the on-disk file).
    pub baseline_gen: u64,
    /// Ribbon undo/redo button state at stash time.
    pub can_undo: bool,
    /// Ribbon undo/redo button state at stash time.
    pub can_redo: bool,
}

/// Open-but-inactive document sessions, keyed by the serialised file token.
pub type DocSessions = HashMap<String, DocSession>;
