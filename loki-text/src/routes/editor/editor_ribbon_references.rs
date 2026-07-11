// SPDX-License-Identifier: Apache-2.0

//! References ribbon tab content (Spec 04 M5, plan 4a.2).
//!
//! The References tab generates a **table of contents** from the document's
//! headings. **Insert** builds a TOC after the caret's block; **Update** rebuilds
//! an existing TOC's cached entries from the current headings (the "update field"
//! action). Both go through the CRDT so they are undoable and relayout
//! immediately; the TOC block flows its cached body in `loki-layout`.

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AT_TOC_INSERT, AT_TOC_UPDATE, AtIcon, AtRibbonGroups, AtRibbonIconButton, RibbonGroupSpec,
    estimate_group_metrics,
};
use dioxus::prelude::*;
use loki_doc_model::content::toc::DEFAULT_TOC_DEPTH;
use loki_doc_model::{first_toc_block_index, insert_table_of_contents, refresh_table_of_contents};
use loki_i18n::fl;

use super::editor_ribbon_layout::apply_and_sync;
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// The caret's top-level block index, or the last block when there is no caret
/// (so an inserted TOC still lands at a sensible place). `None` only with no
/// document loaded.
fn insert_anchor(
    doc_state: &Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
) -> usize {
    if let Some(focus) = cursor_state.peek().focus.as_ref() {
        return focus.paragraph_index;
    }
    super::editor_ribbon_table::block_count(doc_state).saturating_sub(1)
}

/// The document-global index of the first table of contents, if one exists —
/// enables the Update button and targets the refresh.
fn current_toc_index(doc_state: &Arc<Mutex<DocumentState>>) -> Option<usize> {
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;
    first_toc_block_index(&doc.sections)
}

/// Builds the References tab content (a single Table-of-Contents group).
pub(super) fn references_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> Element {
    let ds_insert = Arc::clone(doc_state);
    let ds_update = Arc::clone(doc_state);
    let toc_index = current_toc_index(doc_state);

    let toc_group = RibbonGroupSpec {
        metrics: estimate_group_metrics(1, 2, true),
        label: Some(fl!("ribbon-group-toc")),
        aria_label: fl!("ribbon-group-toc"),
        content: rsx! {
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-toc-insert-aria"),
                is_active:   false,
                is_disabled: false,
                on_click: move |_| {
                    let after = insert_anchor(&ds_insert, cursor_state);
                    let title = fl!("references-toc-title");
                    apply_and_sync(
                        &ds_insert, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                        |l| insert_table_of_contents(l, after, Some(&title), DEFAULT_TOC_DEPTH)
                            .map(|_| ()),
                    );
                },
                AtIcon { path_d: AT_TOC_INSERT.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-toc-update-aria"),
                is_active:   false,
                is_disabled: toc_index.is_none(),
                on_click: move |_| {
                    let Some(idx) = toc_index else { return };
                    let title = fl!("references-toc-title");
                    apply_and_sync(
                        &ds_update, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                        |l| refresh_table_of_contents(l, idx, Some(&title), DEFAULT_TOC_DEPTH),
                    );
                },
                AtIcon { path_d: AT_TOC_UPDATE.to_string() }
            }
        },
    };

    rsx! {
        AtRibbonGroups {
            groups: vec![toc_group],
            overflow_aria_label: fl!("ribbon-overflow-aria"),
        }
    }
}
