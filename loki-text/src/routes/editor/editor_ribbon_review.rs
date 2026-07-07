// SPDX-License-Identifier: Apache-2.0

//! Review ribbon tab content (Spec 04 M5, plan 4a.2).
//!
//! The Review tab hosts **track changes**: a toggle that turns tracked editing
//! on/off. While on, typed text is recorded as a tracked insertion (see
//! `editor_keydown_text` + `Document::insertion_revision`) and rendered
//! underlined in the author's colour. The flag lives in `DocumentSettings` and
//! is persisted through the CRDT (`set_track_changes`), so it survives relayout,
//! undo/redo, and save. Accept/reject controls are added in a later pass.

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AT_CHANGE_ACCEPT, AT_CHANGE_REJECT, AT_TRACK_CHANGES, AtIcon, AtRibbonGroups,
    AtRibbonIconButton, RibbonGroupSpec, estimate_group_metrics,
};
use dioxus::prelude::*;
use loki_doc_model::{
    MutationError, accept_reject_all_revisions, document_track_changes, set_track_changes,
};
use loki_i18n::fl;

use super::editor_ribbon_layout::apply_and_sync;
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// Whether track changes is currently on for the live document.
fn track_changes_on(loro_doc: Signal<Option<loro::LoroDoc>>) -> bool {
    loro_doc.read().as_ref().is_some_and(document_track_changes)
}

/// Whether the document currently holds any tracked change (drives the
/// Accept/Reject buttons' enabled state).
fn has_changes(doc_state: &Arc<Mutex<DocumentState>>) -> bool {
    doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| d.has_tracked_changes()))
        .unwrap_or(false)
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
    let ds_accept = Arc::clone(doc_state);
    let ds_reject = Arc::clone(doc_state);
    let on = track_changes_on(loro_doc);
    let any_changes = has_changes(doc_state);

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
        metrics: estimate_group_metrics(0, 2, true),
        label: Some(fl!("ribbon-group-review-changes")),
        aria_label: fl!("ribbon-group-review-changes"),
        content: rsx! {
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
