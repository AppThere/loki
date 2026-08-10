// SPDX-License-Identifier: Apache-2.0

//! The Insert table dialog (design section 6).
//!
//! Drag-grid at pointer sizes, steppers at touch sizes, **identical options on
//! both** (design note 23). Header row and caption are on by default: an
//! unheaded table is inaccessible and the export pipeline has no way to guess
//! one later (note 24). Row and column operations *after* insertion stay in the
//! Table contextual ribbon tab — this dialog creates, the ribbon edits (note 26).

mod body;
mod grid;
mod preview;
mod spec;

use std::sync::{Arc, Mutex};

use appthere_ui::responsive::use_breakpoint;
use appthere_ui::{AtDialogButton, AtDialogShell, DialogPosture, DialogWidth, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_insert::insert_table_after_cursor;
use super::editor_insert_sync::InsertLinkSync;
use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::set_collapsed_cursor;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use spec::build_table;

pub(in crate::routes::editor) use spec::TableSpec;

/// Props for [`InsertTableDialog`].
#[derive(Clone, Props)]
pub(super) struct InsertTableDialogProps {
    /// Shared document state, for committing the table.
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    /// `Some` while the dialog is open; setting it to `None` closes it.
    pub(super) open: Signal<Option<TableSpec>>,
    /// Loro / undo plumbing (shared with the link dialog).
    pub(super) sync: InsertLinkSync,
}

impl PartialEq for InsertTableDialogProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.open == other.open
            && self.sync == other.sync
    }
}

/// Renders the Insert table dialog.
// PascalCase for rsx; `#[component]` cannot derive the props comparison.
#[allow(non_snake_case)]
pub(super) fn InsertTableDialog(props: InsertTableDialogProps) -> Element {
    let InsertTableDialogProps {
        doc_state,
        mut open,
        sync,
    } = props;
    let posture = DialogPosture::for_breakpoint(use_breakpoint());

    let Some(current) = open.read().clone() else {
        return rsx! {};
    };
    let ds_apply = Arc::clone(&doc_state);

    rsx! {
        AtDialogShell {
            title: fl!("table-dialog-title"),
            close_aria_label: fl!("table-dialog-close-aria"),
            width: DialogWidth::Narrow,
            on_close: move |_| open.set(None),

            body: rsx! { { body::form(open, posture, &current) } },

            footer: rsx! {
                span {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    // Note 26: this dialog creates; the ribbon edits.
                    { fl!("table-dialog-ribbon-note") }
                }
                div {
                    style: format!(
                        "display: flex; flex-direction: {dir}; gap: {gap}px;",
                        dir = if posture.stack_footer { "column-reverse" } else { "row" },
                        gap = tokens::SPACE_2,
                    ),
                    AtDialogButton {
                        label: fl!("table-dialog-cancel"),
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| open.set(None),
                    }
                    AtDialogButton {
                        label: fl!("table-dialog-insert"),
                        primary: true,
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| {
                            let Some(s) = open.read().clone() else { return };
                            if insert_table(&ds_apply, &sync, &s) {
                                open.set(None);
                            }
                        },
                    }
                }
            },
        }
    }
}

/// Inserts the table `spec` describes after the caret, returning whether it
/// landed.
fn insert_table(
    doc_state: &Arc<Mutex<DocumentState>>,
    sync: &InsertLinkSync,
    spec: &TableSpec,
) -> bool {
    let guard = sync.loro_doc.read();
    let Some(ldoc) = guard.as_ref() else {
        return false;
    };
    let cursor = sync.cursor_state.read().clone();
    // The same insertion the ribbon used, given a configured table instead of a
    // bare grid — one tested path for placing a table and finding its caret.
    let Ok(Some(caret)) = insert_table_after_cursor(ldoc, &cursor, build_table(spec)) else {
        return false;
    };
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
    // Park the caret in the first cell — a table you have to click into before
    // typing is a table inserted twice. After the relayout, so the position
    // resolves against the fresh pages.
    set_collapsed_cursor(doc_state, sync.cursor_state, caret);
    true
}
