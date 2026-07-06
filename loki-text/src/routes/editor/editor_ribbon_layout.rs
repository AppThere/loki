// SPDX-License-Identifier: Apache-2.0

//! Layout ribbon tab content (Spec 04 M5, plan 4a.2).
//!
//! The first Layout control is **page orientation**. Portrait/Landscape apply
//! [`set_document_orientation`] to every section (swapping page width/height)
//! and relayout, so the document immediately re-flows at the new page size.

use std::sync::{Arc, Mutex};

use appthere_ui::{AT_PAGE_LANDSCAPE, AT_PAGE_PORTRAIT, AtIcon, AtRibbonGroup, AtRibbonIconButton};
use dioxus::prelude::*;
use loki_doc_model::{document_is_landscape, set_document_orientation};
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Applies `landscape` orientation to the document, relays out, and syncs
/// undo/redo.
fn set_orientation(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    landscape: bool,
) {
    {
        let guard = loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        if set_document_orientation(ldoc, landscape).is_err() {
            return;
        }
        apply_mutation_and_relayout(doc_state, ldoc);
    }
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
}

/// Builds the Layout tab content (currently the Orientation group).
pub(super) fn layout_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> Element {
    let landscape = loro_doc.read().as_ref().is_some_and(document_is_landscape);
    let ds_portrait = Arc::clone(doc_state);
    let ds_landscape = Arc::clone(doc_state);

    rsx! {
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-orientation")),
            aria_label: fl!("ribbon-group-orientation"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-orientation-portrait-aria"),
                is_active:   !landscape,
                is_disabled: false,
                on_click: move |_| set_orientation(
                    &ds_portrait, loro_doc, cursor_state, undo_manager, can_undo, can_redo, false,
                ),
                AtIcon { path_d: AT_PAGE_PORTRAIT.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-orientation-landscape-aria"),
                is_active:   landscape,
                is_disabled: false,
                on_click: move |_| set_orientation(
                    &ds_landscape, loro_doc, cursor_state, undo_manager, can_undo, can_redo, true,
                ),
                AtIcon { path_d: AT_PAGE_LANDSCAPE.to_string() }
            }
        }
    }
}
