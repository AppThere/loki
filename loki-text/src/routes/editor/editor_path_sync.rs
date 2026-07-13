// SPDX-License-Identifier: Apache-2.0

//! Path-change detection and per-document state handover for
//! [`super::editor_inner::EditorInner`].
//!
//! On tab switch the outgoing document's live state (CRDT, undo history,
//! layout, cursor) is stashed into the app-level [`DocSessions`] map instead
//! of being discarded, and the incoming document's session is restored if one
//! exists — unsaved edits survive tab switches.  Only documents with no
//! stashed session fall through to the reset path (and are then loaded from
//! disk by `use_resource`).

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use super::editor_state::{ColorPickerTarget, SaveStatus, StyleDraft};
use crate::editing::cursor::CursorState;
use crate::editing::saved_state::SavedStateHandle;
use crate::editing::state::DocumentState;
use crate::sessions::{DocSession, DocSessions};
use crate::tabs::OpenTab;

/// All per-document signals reset or restored on tab switch, grouped to keep
/// the [`sync_path_and_reset`] signature manageable.
pub(super) struct PathSyncSignals {
    pub cursor_state: Signal<CursorState>,
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    pub total_pages: Signal<u32>,
    pub current_page: Signal<u32>,
    pub can_undo: Signal<bool>,
    pub can_redo: Signal<bool>,
    pub font_panel_open: Signal<bool>,
    pub is_style_picker_open: Signal<bool>,
    pub open_color_picker: Signal<Option<ColorPickerTarget>>,
    pub editing_style_draft: Signal<Option<StyleDraft>>,
    pub save_message: Signal<Option<SaveStatus>>,
    pub baseline_gen: Signal<u64>,
    pub saved_state: Signal<SavedStateHandle>,
}

/// Synchronises `path_signal` with the `path` prop.  On change, stashes the
/// outgoing document's session, then either restores the incoming document's
/// stashed session or resets all per-document signals for a fresh disk load.
pub(super) fn sync_path_and_reset(
    path: &str,
    path_signal: &mut Signal<String>,
    doc_state: &Arc<Mutex<DocumentState>>,
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

    stash_outgoing(&current, doc_state, tabs, &mut sessions, sig);

    // Transient UI state never carries across documents.
    sig.font_panel_open.set(false);
    sig.is_style_picker_open.set(false);
    sig.open_color_picker.set(None);
    sig.editing_style_draft.set(None);
    sig.save_message.set(None);

    let restored = sessions.write().remove(path);
    match restored {
        Some(session) => restore_session(session, doc_state, sig, *path_signal),
        None => reset_for_fresh_load(doc_state, sig),
    }
}

/// Move the outgoing document's live state into the session map. No-op when no
/// document ever finished loading, or when no tab points at `old_path` any more
/// — a closed (or Save-As-repointed) tab must not resurrect its old state on
/// reopen. The liveness guard lives here so both call sites (path change and
/// the unmount hook) are covered uniformly.
///
/// Called on path change (doc → doc tab switch) and from the unmount hook in
/// `EditorInner` (doc → Home navigation unmounts the editor route).
pub(super) fn stash_outgoing(
    old_path: &str,
    doc_state: &Arc<Mutex<DocumentState>>,
    tabs: Signal<Vec<OpenTab>>,
    sessions: &mut Signal<DocSessions>,
    sig: &mut PathSyncSignals,
) {
    let Some(loro_doc) = sig.loro_doc.write().take() else {
        return; // nothing loaded — nothing to stash
    };
    let undo_manager = sig.undo_manager.write().take();
    if !tabs.peek().iter().any(|t| t.path == old_path) {
        return; // tab closed or Save-As-repointed — do not resurrect on reopen
    }
    // The layout is deliberately NOT stashed (memory F3 / plan 6.1): it is
    // recomputed from `document` on restore, so an inactive tab retains only
    // the model. The Arc in `DocumentState` drops on the incoming reset/load.
    let (document, generation) = match doc_state.lock() {
        Ok(s) => (s.document.clone(), s.generation),
        Err(_) => {
            tracing::error!("doc_state lock poisoned during stash — session dropped");
            return;
        }
    };
    sessions.write().insert(
        old_path.to_owned(),
        DocSession {
            loro_doc,
            undo_manager,
            document,
            generation,
            cursor: sig.cursor_state.peek().clone(),
            baseline_gen: *sig.baseline_gen.peek(),
            saved_state: sig.saved_state.peek().clone(),
            can_undo: *sig.can_undo.peek(),
            can_redo: *sig.can_redo.peek(),
        },
    );
}

/// Write a stashed session back into the live editor state and kick off the
/// relayout that replaces the deliberately-unstashed layout (memory F3):
/// the canvas shows the loading indicator (`total_pages == 0`, exactly like
/// the fresh-open path) until the worker publishes.
///
/// Called on path change and from the mount hook in `EditorInner` (returning
/// to a document tab after the editor route was unmounted by Home). Both
/// sites run in component scope, so the relayout task can be spawned here.
pub(super) fn restore_session(
    session: DocSession,
    doc_state: &Arc<Mutex<DocumentState>>,
    sig: &mut PathSyncSignals,
    path_signal: Signal<String>,
) {
    let document = session.document.clone();
    if let Ok(mut state) = doc_state.lock() {
        state.document = session.document;
        state.generation = session.generation;
        state.page_count = 0;
        state.paginated_layout = None;
    } else {
        tracing::error!("doc_state lock poisoned during restore — state may be stale");
    }
    sig.cursor_state.set(session.cursor);
    sig.loro_doc.set(Some(session.loro_doc));
    sig.undo_manager.set(session.undo_manager);
    sig.total_pages.set(0);
    sig.current_page.set(1);
    sig.can_undo.set(session.can_undo);
    sig.can_redo.set(session.can_redo);
    sig.baseline_gen.set(session.baseline_gen);
    sig.saved_state.set(session.saved_state);

    super::editor_layout_task::spawn_restore_relayout(
        Arc::clone(doc_state),
        document,
        session.generation,
        path_signal,
        sig.total_pages,
    );
}

/// Reset all per-document state ahead of a fresh `load_document` pass.
fn reset_for_fresh_load(doc_state: &Arc<Mutex<DocumentState>>, sig: &mut PathSyncSignals) {
    if let Ok(mut state) = doc_state.lock() {
        state.document = None;
        state.generation = 0;
        state.page_count = 0;
        state.paginated_layout = None;
    } else {
        tracing::error!("doc_state lock poisoned during tab switch — state may be stale");
    }
    sig.cursor_state.set(CursorState::default());
    sig.loro_doc.set(None);
    sig.undo_manager.set(None);
    sig.total_pages.set(0);
    sig.current_page.set(1);
    sig.can_undo.set(false);
    sig.can_redo.set(false);
    sig.saved_state.set(SavedStateHandle::new());
}
