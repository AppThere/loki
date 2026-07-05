// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-document session stash/restore for tab switches (audit F1 residual /
//! plan 4b.6) — see [`crate::sessions`] for the session map itself.
//!
//! Extracted from `editor_inner.rs` to keep that file under the 300-line
//! ceiling.

use dioxus::prelude::*;
use loki_presentation_model::Presentation;

use crate::sessions::{DocSession, DocSessions};
use crate::tabs::OpenTab;

/// Moves the live editable state into the session map. No-op when nothing is
/// loaded, or when no tab points at `old_path` any more — a closed (or
/// Save-As-repointed) tab must not resurrect its old state on reopen.
pub(super) fn stash_outgoing(
    old_path: &str,
    tabs: Signal<Vec<OpenTab>>,
    mut doc_sessions: Signal<DocSessions>,
    mut doc: Signal<Option<Presentation>>,
    active_idx: Signal<usize>,
    dirty: Signal<bool>,
) {
    let Some(pres) = doc.write().take() else {
        return;
    };
    if !tabs.peek().iter().any(|t| t.path == old_path) {
        return;
    }
    doc_sessions.write().insert(
        old_path.to_owned(),
        DocSession {
            doc: pres,
            active_idx: *active_idx.peek(),
            dirty: *dirty.peek(),
        },
    );
}

/// Writes a stashed session back into the live editor state. Returns whether
/// a session existed (`false` → the caller resets for a fresh disk load).
pub(super) fn restore_session(
    new_path: &str,
    mut doc_sessions: Signal<DocSessions>,
    mut doc: Signal<Option<Presentation>>,
    mut active_idx: Signal<usize>,
    mut dirty: Signal<bool>,
) -> bool {
    let Some(session) = doc_sessions.write().remove(new_path) else {
        return false;
    };
    doc.set(Some(session.doc));
    active_idx.set(session.active_idx);
    dirty.set(session.dirty);
    true
}
