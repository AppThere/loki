// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Saved-state tracking over the undo stack (plan 4b.3, `undo-dirty`).
//!
//! The tab dirty indicator needs to answer "does the live document match the
//! file on disk?". A generation counter cannot: it only moves forward, so
//! undoing back to the saved state still reads as dirty. This module tracks
//! the *undo-stack depth* of the last save instead, giving the classic
//! clean-index semantics (Qt `QUndoStack::setClean` and friends):
//!
//! - saving records the current depth as the clean point;
//! - undo/redo moving the stack back to that depth means the document again
//!   equals the file — **clean**;
//! - a *fresh edit* made below the clean depth truncates the redo stack that
//!   led back to it, so the saved state becomes permanently **unreachable**
//!   (dirty until the next save).
//!
//! The tracker mirrors the stack depth via the [`loro::UndoManager`]
//! `on_push`/`on_pop` hooks. The discriminator for "fresh edit vs. redo
//! replay" (both push onto the undo stack) is loro's third `on_push`
//! argument: a fresh local edit passes `Some(DiffEvent)`, while the pushes
//! performed *inside* `undo()`/`redo()` pass `None`.
//!
//! Two usage invariants keep that discriminator sound (both already hold in
//! this crate — see `post_mutation_sync`):
//!
//! 1. every mutation is committed before `undo()`/`redo()`/save runs, so
//!    loro's internal `record_new_checkpoint` calls never coalesce pending
//!    ops into an item (which would also arrive with `None`);
//! 2. no merge interval or undo grouping is configured, so every edit pushes
//!    its own item and the clean depth can never end up *inside* an item.

use std::sync::{Arc, Mutex};

use loro::{UndoItemMeta, UndoOrRedo};

#[cfg(test)]
#[path = "saved_state_tests.rs"]
mod tests;

/// Depth bookkeeping shared between the editor and the hooks installed on
/// the paired [`loro::UndoManager`].
#[derive(Debug)]
struct Tracker {
    /// Undo-stack depth at the last save; `None` = the saved state is no
    /// longer reachable by undo/redo.
    saved_depth: Option<usize>,
    /// Mirror of the undo-stack depth.
    depth: usize,
}

impl Tracker {
    /// A fresh local edit pushed a new undo item. An edit at or below the
    /// clean depth cleared the redo path back to the saved state.
    fn on_fresh_edit(&mut self) {
        self.depth += 1;
        if let Some(d) = self.saved_depth
            && self.depth <= d
        {
            self.saved_depth = None;
        }
    }
}

/// Cloneable handle to one document's saved-state tracker.
///
/// [`attach`](Self::attach) installs the mirroring hooks on the document's
/// `UndoManager`; the handle then travels with the editor signals (and the
/// tab's stashed `DocSession`) so the dirty indicator can query
/// [`is_clean`](Self::is_clean) at any time.
#[derive(Clone, Debug)]
pub struct SavedStateHandle(Arc<Mutex<Tracker>>);

impl SavedStateHandle {
    /// A tracker for a freshly loaded document: depth 0 is the clean point
    /// (the document matches what was read from disk).
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Tracker {
            saved_depth: Some(0),
            depth: 0,
        })))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Tracker> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Installs the `on_push`/`on_pop` mirroring hooks. Call exactly once per
    /// `UndoManager` (a replacement manager needs a fresh handle).
    pub fn attach(&self, um: &mut loro::UndoManager) {
        let push_state = Arc::clone(&self.0);
        um.set_on_push(Some(Box::new(move |kind, _span, event| {
            if kind == UndoOrRedo::Undo {
                let mut t = push_state.lock().unwrap_or_else(|e| e.into_inner());
                if event.is_some() {
                    t.on_fresh_edit();
                } else {
                    // redo() replaying an item back onto the undo stack —
                    // the path to the saved state is preserved.
                    t.depth += 1;
                }
            }
            UndoItemMeta::default()
        })));
        let pop_state = Arc::clone(&self.0);
        um.set_on_pop(Some(Box::new(move |kind, _span, _meta| {
            if kind == UndoOrRedo::Undo {
                // undo() popping the undo stack (including no-op items it
                // pops while searching for an effective one).
                let mut t = pop_state.lock().unwrap_or_else(|e| e.into_inner());
                t.depth = t.depth.saturating_sub(1);
            }
        })));
    }

    /// Records the current depth as the clean point (call on save success).
    pub fn mark_saved(&self) {
        let mut t = self.lock();
        t.saved_depth = Some(t.depth);
    }

    /// Whether the document currently equals the last-saved state.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        let t = self.lock();
        t.saved_depth == Some(t.depth)
    }
}

impl Default for SavedStateHandle {
    fn default() -> Self {
        Self::new()
    }
}
