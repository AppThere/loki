// SPDX-License-Identifier: Apache-2.0

//! The document metadata dialog (design section 4).
//!
//! The eighteen editable Dublin Core fields the model carries, split across
//! five tabs, plus the EPUB accessibility metadata the publish step wants and
//! a read-only statistics tab.
//!
//! # It edits the existing draft
//!
//! The field set, the read, and the write-back are
//! [`super::editor_metadata`]'s — the same [`MetaDraft`] the inline Publish-tab
//! panel uses. This dialog is a second *presentation* of that draft, not a
//! second copy of it: a duplicate field list would drift from the model's on
//! the first field either gained.

mod body;
mod stats;
mod tab_review;
mod tabs;

use std::sync::{Arc, Mutex};

use appthere_ui::responsive::use_breakpoint;
use appthere_ui::{
    AtDialogButton, AtDialogShell, AtDialogTabStrip, DialogPosture, DialogWidth, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_metadata::{MetaDraft, apply_meta_draft, meta_to_draft};
use super::editor_style_editor::StyleEditorSync;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use tabs::{MetaTab, missing_required};

/// Props for [`MetadataDialog`].
#[derive(Clone, Props)]
pub(super) struct MetadataDialogProps {
    /// Shared document state.
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    /// `true` while the dialog is open.
    pub(super) open: Signal<bool>,
    /// Loro / undo plumbing.
    pub(super) sync: StyleEditorSync,
}

impl PartialEq for MetadataDialogProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.open == other.open
            && self.sync == other.sync
    }
}

/// Renders the metadata dialog.
// PascalCase for rsx; `#[component]` cannot derive the props comparison.
#[allow(non_snake_case)]
pub(super) fn MetadataDialog(props: MetadataDialogProps) -> Element {
    let MetadataDialogProps {
        doc_state,
        mut open,
        sync,
    } = props;
    let posture = DialogPosture::for_breakpoint(use_breakpoint());

    // Seeded once per mount: the dialog is mounted behind `open`, so closing it
    // unmounts the scope and the next open re-reads the document.
    let mut draft = use_signal(|| Some(meta_to_draft(&doc_state)));
    let mut active_tab = use_signal(|| MetaTab::General);
    let menu_open = use_signal(|| false);

    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let missing = missing_required(&current.values);
    let tab = *active_tab.read();
    let ds_apply = Arc::clone(&doc_state);

    rsx! {
        AtDialogShell {
            title: fl!("meta-dialog-title"),
            subtitle: rsx! {
                div {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { fl!("meta-dialog-subtitle") }
                }
            },
            close_aria_label: fl!("meta-dialog-close-aria"),
            width: DialogWidth::Wide,
            on_close: move |_| open.set(false),

            tabs: rsx! {
                AtDialogTabStrip {
                    labels: MetaTab::labels(),
                    active: tab.index(),
                    inline_at_medium: MetaTab::INLINE_AT_MEDIUM,
                    menu_open,
                    more_label: fl!("meta-dialog-tabs-more"),
                    on_select: move |idx: usize| active_tab.set(MetaTab::from_index(idx)),
                }
            },

            body: rsx! { { body::tab_body(tab, &doc_state, draft, posture, missing) } },

            footer: rsx! {
                span {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { body::revision_line(&doc_state) }
                }
                div {
                    style: format!(
                        "display: flex; flex-direction: {dir}; gap: {gap}px;",
                        dir = if posture.stack_footer { "column-reverse" } else { "row" },
                        gap = tokens::SPACE_2,
                    ),
                    AtDialogButton {
                        label: fl!("meta-dialog-cancel"),
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| open.set(false),
                    }
                    AtDialogButton {
                        label: fl!("meta-dialog-save"),
                        primary: true,
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| {
                            let Some(d) = draft.read().clone() else { return };
                            if save(&ds_apply, &sync, &d) {
                                // Re-seed so the fields show what was committed
                                // — the write trims, and a field the user left
                                // padded should settle visibly.
                                draft.set(Some(meta_to_draft(&ds_apply)));
                                open.set(false);
                            }
                        },
                    }
                }
            },
        }
    }
}

/// Persists `draft` through Loro, returning whether it landed.
fn save(doc_state: &Arc<Mutex<DocumentState>>, sync: &StyleEditorSync, draft: &MetaDraft) -> bool {
    let guard = sync.loro_doc.read();
    let Some(ldoc) = guard.as_ref() else {
        return false;
    };
    apply_meta_draft(ldoc, doc_state, draft);
    apply_mutation_and_relayout(doc_state, ldoc);
    drop(guard);
    post_mutation_sync(
        doc_state,
        sync.loro_doc,
        sync.cursor_state,
        sync.undo_manager,
        sync.can_undo,
        sync.can_redo,
    );
    true
}
