// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Post-save CRDT history compaction (memory-audit Finding 6).
//!
//! A Loro oplog grows with every keystroke, so a long session retains
//! unbounded history. The save point is the natural horizon: the file is now
//! the durable state, so history behind it only serves undo. After each
//! successful save this module either
//!
//! - **truncates** the history (`compact_history`) when the oplog has grown
//!   past [`COMPACT_THRESHOLD_OPS`] — swapping in a fresh doc and resetting
//!   the undo stack to the save point, or
//! - **re-encodes** it in place (`compact_in_place`) below the threshold —
//!   free memory savings with undo fully preserved.
//!
//! The threshold keeps the undo-reset tradeoff rare: routine saves keep
//! their undo history; only marathon sessions pay with a truncated stack.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loro::LoroDoc;

use crate::editing::state::DocumentState;

/// Oplog size (in ops) above which a save triggers full history truncation.
/// ~2 ops per keystroke → roughly 10k keystrokes since the last truncation.
const COMPACT_THRESHOLD_OPS: usize = 20_000;

/// Compacts the CRDT history after a successful save. See the module docs
/// for the threshold behaviour.
pub(super) fn compact_after_save(
    mut loro_doc: Signal<Option<LoroDoc>>,
    mut undo_manager: Signal<Option<loro::UndoManager>>,
    mut saved_state: Signal<crate::editing::saved_state::SavedStateHandle>,
    mut can_undo: Signal<bool>,
    mut can_redo: Signal<bool>,
    doc_state: &Arc<Mutex<DocumentState>>,
) {
    // Clone the handle (cheap, shared) so the read guard drops before set().
    let Some(doc) = loro_doc.peek().clone() else {
        return;
    };

    if doc.len_ops() < COMPACT_THRESHOLD_OPS {
        loki_doc_model::loro_bridge::compact_in_place(&doc);
        return;
    }

    match loki_doc_model::loro_bridge::compact_history(&doc) {
        Ok(fresh) => {
            // Everything bound to the old doc instance is recreated: the
            // undo manager restarts at the save point, the clean-checkpoint
            // tracker restarts at depth 0 (= the just-saved state), and the
            // incremental reader re-seeds from the new doc on the next edit.
            let mut um = loro::UndoManager::new(&fresh);
            let tracker = crate::editing::saved_state::SavedStateHandle::new();
            tracker.attach(&mut um);
            saved_state.set(tracker);
            loro_doc.set(Some(fresh));
            undo_manager.set(Some(um));
            can_undo.set(false);
            can_redo.set(false);
            let mut state = doc_state.lock().unwrap_or_else(|e| e.into_inner());
            state.incremental = None;
        }
        Err(err) => {
            // The save itself succeeded; a failed compaction only means the
            // memory win is skipped this time. Keep the working doc.
            tracing::warn!("post-save history compaction failed: {err}");
            loki_doc_model::loro_bridge::compact_in_place(&doc);
        }
    }
}
