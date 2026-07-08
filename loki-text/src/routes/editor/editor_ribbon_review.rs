// SPDX-License-Identifier: Apache-2.0

//! Review ribbon tab content (Spec 04 M5, plan 4a.2).
//!
//! The Review tab hosts **track changes**: a toggle that turns tracked editing
//! on/off. While on, typed text is recorded as a tracked insertion (see
//! `editor_keydown_text` + `Document::insertion_revision`) and rendered
//! underlined in the author's colour. The flag lives in `DocumentSettings` and
//! is persisted through the CRDT (`set_track_changes`), so it survives relayout,
//! undo/redo, and save. The Changes group resolves revisions: **Accept / Reject**
//! act on the single change at the caret (`accept_reject_revision_at`), and
//! **Accept all / Reject all** sweep the whole document (`accept_reject_all_revisions`).

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AT_CHANGE_ACCEPT, AT_CHANGE_ACCEPT_ONE, AT_CHANGE_REJECT, AT_CHANGE_REJECT_ONE,
    AT_TRACK_CHANGES, AtIcon, AtRibbonGroups, AtRibbonIconButton, RibbonGroupSpec,
    estimate_group_metrics,
};
use dioxus::prelude::*;
use loki_doc_model::{
    MutationError, accept_reject_all_revisions, accept_reject_revision_at, document_track_changes,
    revision_at, set_track_changes,
};
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::set_collapsed_cursor;
use super::editor_ribbon_layout::apply_and_sync;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Whether track changes is currently on for the live document.
fn track_changes_on(loro_doc: Signal<Option<loro::LoroDoc>>) -> bool {
    loro_doc.read().as_ref().is_some_and(document_track_changes)
}

/// Whether the document currently holds any tracked change (drives the
/// Accept-all / Reject-all buttons' enabled state).
fn has_changes(doc_state: &Arc<Mutex<DocumentState>>) -> bool {
    doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| d.has_tracked_changes()))
        .unwrap_or(false)
}

/// Whether a tracked change sits at the caret (drives the per-change buttons).
fn change_at_caret(
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
) -> bool {
    let Some(focus) = cursor_state.read().focus.clone() else {
        return false;
    };
    loro_doc
        .read()
        .as_ref()
        .is_some_and(|l| revision_at(l, &focus.block_path(), focus.byte_offset))
}

/// Accepts/rejects the single tracked change at the caret, then repositions the
/// caret (a removal shifts the text). A no-op when the caret is not on a change.
#[allow(clippy::too_many_arguments)] // mirrors the keydown helpers' signal set
fn accept_reject_at_caret(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    accept: bool,
) {
    let Some(focus) = cursor_state.peek().focus.clone() else {
        return;
    };
    let path = focus.block_path();
    let new_off = {
        let guard = loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        let Ok(Some(off)) = accept_reject_revision_at(ldoc, &path, focus.byte_offset, accept)
        else {
            return; // no change at the caret, or an error — nothing mutated
        };
        apply_mutation_and_relayout(doc_state, ldoc);
        off
    };
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    set_collapsed_cursor(
        doc_state,
        cursor_state,
        DocumentPosition {
            byte_offset: new_off,
            ..focus
        },
    );
}

/// Builds the Review tab content (a single Tracking group with the toggle).
pub(super) fn review_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> Element {
    let ds = Arc::clone(doc_state);
    let ds_accept_one = Arc::clone(doc_state);
    let ds_reject_one = Arc::clone(doc_state);
    let ds_accept = Arc::clone(doc_state);
    let ds_reject = Arc::clone(doc_state);
    let on = track_changes_on(loro_doc);
    let any_changes = has_changes(doc_state);
    let on_change = change_at_caret(loro_doc, cursor_state);

    let track_group = RibbonGroupSpec {
        metrics: estimate_group_metrics(1, 1, true),
        label: Some(fl!("ribbon-group-review-track")),
        aria_label: fl!("ribbon-group-review-track"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-track-changes-aria"),
                is_active:   on,
                is_disabled: false,
                on_click: move |_| apply_and_sync(
                    &ds, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                    |l| set_track_changes(l, !on).map_err(|e| MutationError::Loro(e.to_string())),
                ),
                AtIcon { path_d: AT_TRACK_CHANGES.to_string() }
            }
        },
    };

    let changes_group = RibbonGroupSpec {
        metrics: estimate_group_metrics(0, 4, true),
        label: Some(fl!("ribbon-group-review-changes")),
        aria_label: fl!("ribbon-group-review-changes"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-accept-change-aria"),
                is_active:   false,
                is_disabled: !on_change,
                on_click: move |_| accept_reject_at_caret(
                    &ds_accept_one, loro_doc, cursor_state, undo_manager, can_undo, can_redo, true,
                ),
                AtIcon { path_d: AT_CHANGE_ACCEPT_ONE.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-reject-change-aria"),
                is_active:   false,
                is_disabled: !on_change,
                on_click: move |_| accept_reject_at_caret(
                    &ds_reject_one, loro_doc, cursor_state, undo_manager, can_undo, can_redo, false,
                ),
                AtIcon { path_d: AT_CHANGE_REJECT_ONE.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-accept-all-aria"),
                is_active:   false,
                is_disabled: !any_changes,
                on_click: move |_| apply_and_sync(
                    &ds_accept, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                    |l| accept_reject_all_revisions(l, true).map(|_| ()),
                ),
                AtIcon { path_d: AT_CHANGE_ACCEPT.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-reject-all-aria"),
                is_active:   false,
                is_disabled: !any_changes,
                on_click: move |_| apply_and_sync(
                    &ds_reject, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                    |l| accept_reject_all_revisions(l, false).map(|_| ()),
                ),
                AtIcon { path_d: AT_CHANGE_REJECT.to_string() }
            }
        },
    };

    rsx! {
        AtRibbonGroups {
            groups: vec![track_group, changes_group],
            overflow_aria_label: fl!("ribbon-overflow-aria"),
        }
    }
}
