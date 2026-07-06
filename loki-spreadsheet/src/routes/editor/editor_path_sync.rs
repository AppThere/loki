// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Path-change detection and per-document state handover for
//! [`super::editor_inner::EditorInner`].
//!
//! On tab switch the outgoing workbook's live state (CRDT, undo history, grid
//! snapshot, selection) is stashed into the app-level
//! [`DocSessions`] map instead of being discarded, and the
//! incoming workbook's session is restored if one exists — unsaved edits
//! survive tab switches (plan 4b.6, mirrors `loki-text`). Only documents with
//! no stashed session fall through to the reset path (and are then loaded
//! from disk by `use_resource`).

use dioxus::prelude::*;

use crate::sessions::{DocSession, DocSessions};
use crate::tabs::OpenTab;

/// All per-document signals reset or restored on tab switch, grouped to keep
/// the [`sync_path_and_reset`] signature manageable.
pub(super) struct PathSyncSignals {
    pub workbook_snap: Signal<loki_sheet_model::Workbook>,
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    pub can_undo: Signal<bool>,
    pub can_redo: Signal<bool>,
    pub selected_cell: Signal<Option<(usize, usize)>>,
    pub editing_cell: Signal<Option<(usize, usize)>>,
}

/// Synchronises `path_signal` with the `path` prop. On change, stashes the
/// outgoing document's session, then either restores the incoming document's
/// stashed session or resets all per-document signals for a fresh disk load.
pub(super) fn sync_path_and_reset(
    path: &str,
    path_signal: &mut Signal<String>,
    tabs: Signal<Vec<OpenTab>>,
    mut sessions: Signal<DocSessions>,
    sig: &mut PathSyncSignals,
) {
    let current = path_signal.peek().clone();
    if current == path {
        return;
    }
    tracing::debug!(
        "EditorInner: path changed from {} to {} → stashing outgoing session",
        current,
        path
    );
    path_signal.set(path.to_owned());

    stash_outgoing(&current, tabs, sessions, sig);

    // Transient UI state never carries across documents.
    sig.editing_cell.set(None);

    let restored = sessions.write().remove(path);
    match restored {
        Some(session) => restore_session(session, sig),
        None => reset_for_fresh_load(sig),
    }
}

/// Move the outgoing document's live state into the session map. No-op when
/// no document ever finished loading, or when no tab points at `old_path` any
/// more — a closed (or Save-As-repointed) tab must not resurrect its old
/// state on reopen.
///
/// Called on path change (doc → doc tab switch) and from the unmount hook in
/// `EditorInner` (doc → Home navigation unmounts the editor route).
pub(super) fn stash_outgoing(
    old_path: &str,
    tabs: Signal<Vec<OpenTab>>,
    mut sessions: Signal<DocSessions>,
    sig: &mut PathSyncSignals,
) {
    let Some(loro_doc) = sig.loro_doc.write().take() else {
        return; // nothing loaded — nothing to stash
    };
    let undo_manager = sig.undo_manager.write().take();
    if !tabs.peek().iter().any(|t| t.path == old_path) {
        return;
    }
    sessions.write().insert(
        old_path.to_owned(),
        DocSession {
            loro_doc,
            undo_manager,
            workbook: sig.workbook_snap.peek().clone(),
            can_undo: *sig.can_undo.peek(),
            can_redo: *sig.can_redo.peek(),
            selected_cell: *sig.selected_cell.peek(),
        },
    );
}

/// Write a stashed session back into the live editor state.
///
/// Called on path change and from the mount hook in `EditorInner` (returning
/// to a workbook tab after the editor route was unmounted by Home).
pub(super) fn restore_session(session: DocSession, sig: &mut PathSyncSignals) {
    sig.workbook_snap.set(session.workbook);
    sig.loro_doc.set(Some(session.loro_doc));
    sig.undo_manager.set(session.undo_manager);
    sig.can_undo.set(session.can_undo);
    sig.can_redo.set(session.can_redo);
    sig.selected_cell.set(session.selected_cell);
}

/// Reset all per-document state ahead of a fresh `load_document` pass.
fn reset_for_fresh_load(sig: &mut PathSyncSignals) {
    sig.workbook_snap.set(loki_sheet_model::Workbook::new());
    sig.loro_doc.set(None);
    sig.undo_manager.set(None);
    sig.can_undo.set(false);
    sig.can_redo.set(false);
    sig.selected_cell.set(Some((0, 0)));
}
