// SPDX-License-Identifier: Apache-2.0

//! The Insert link dialog (design section 5).
//!
//! Small enough to skip tabs entirely: a **segmented control** over the four
//! target kinds replaces them, and the body below changes with the kind
//! (design note 19). In-document targets are picked from the live outline
//! rather than typed (note 20), the link's appearance comes from a character
//! style so it participates in the same inheritance as everything else (note
//! 21), and address validation is inline and non-blocking (note 22).

mod body;
mod target;
mod targets;
mod validate;

use std::sync::{Arc, Mutex};

use appthere_ui::responsive::use_breakpoint;
use appthere_ui::{AtDialogButton, AtDialogShell, DialogPosture, DialogWidth, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_insert::set_hyperlink;
use super::editor_insert_panel::InsertLinkSync;
use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use target::LinkKind;
use validate::{to_url, validate};

/// The dialog's working state — the address per kind, plus the display text.
///
/// One buffer **per kind**, not one shared buffer: switching from Web to Email
/// to compare two destinations and finding the first one gone is a small,
/// avoidable loss, and the kinds' addresses are not interchangeable anyway.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::routes::editor) struct LinkDraft {
    /// Address typed for the Web kind.
    pub web: String,
    /// Anchor picked for the in-document kind.
    pub document: String,
    /// Address typed for the Email kind.
    pub email: String,
    /// Path typed for the File kind.
    pub file: String,
    /// The text the link will show. Seeded from the selection.
    pub display_text: String,
    /// The optional screen-reader description.
    pub description: String,
    /// The filter over the in-document target list.
    pub filter: String,
}

impl LinkDraft {
    /// The address buffer for `kind`.
    #[must_use]
    pub fn address(&self, kind: LinkKind) -> &str {
        match kind {
            LinkKind::Web => &self.web,
            LinkKind::Document => &self.document,
            LinkKind::Email => &self.email,
            LinkKind::File => &self.file,
        }
    }

    /// Replaces the address buffer for `kind`.
    pub fn set_address(&mut self, kind: LinkKind, value: String) {
        match kind {
            LinkKind::Web => self.web = value,
            LinkKind::Document => self.document = value,
            LinkKind::Email => self.email = value,
            LinkKind::File => self.file = value,
        }
    }
}

/// Props for [`InsertLinkDialog`].
#[derive(Clone, Props)]
pub(super) struct InsertLinkDialogProps {
    /// Shared document state, for reading the outline.
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    /// `Some` while the dialog is open; setting it to `None` closes it.
    pub(super) open: Signal<Option<LinkDraft>>,
    /// Loro / undo plumbing.
    pub(super) sync: InsertLinkSync,
}

impl PartialEq for InsertLinkDialogProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.open == other.open
            && self.sync == other.sync
    }
}

/// Renders the Insert link dialog.
// PascalCase for rsx; `#[component]` cannot derive the props comparison.
#[allow(non_snake_case)]
pub(super) fn InsertLinkDialog(props: InsertLinkDialogProps) -> Element {
    let InsertLinkDialogProps {
        doc_state,
        mut open,
        sync,
    } = props;
    let posture = DialogPosture::for_breakpoint(use_breakpoint());
    let kind = use_signal(|| LinkKind::Web);

    let Some(draft) = open.read().clone() else {
        return rsx! {};
    };
    let current = *kind.read();
    let address = draft.address(current).to_string();
    let state = validate(current, &address);
    let can_insert = state.can_insert();
    let ds_apply = Arc::clone(&doc_state);

    rsx! {
        AtDialogShell {
            title: fl!("link-dialog-title"),
            close_aria_label: fl!("link-dialog-close-aria"),
            width: DialogWidth::Narrow,
            on_close: move |_| open.set(None),

            body: rsx! {
                { body::form(&doc_state, current, kind, open, posture, &state) }
            },

            footer: rsx! {
                span {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { fl!("link-dialog-export-note") }
                }
                div {
                    style: format!(
                        "display: flex; flex-direction: {dir}; gap: {gap}px;",
                        dir = if posture.stack_footer { "column-reverse" } else { "row" },
                        gap = tokens::SPACE_2,
                    ),
                    AtDialogButton {
                        label: fl!("link-dialog-cancel"),
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| open.set(None),
                    }
                    AtDialogButton {
                        label: fl!("link-dialog-insert"),
                        primary: true,
                        // Note 22: a malformed address dims Insert rather than
                        // throwing an alert the user has to dismiss.
                        disabled: !can_insert,
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| {
                            let Some(d) = open.read().clone() else { return };
                            let url = to_url(current, d.address(current));
                            if apply_link(&ds_apply, &sync, &url) {
                                open.set(None);
                            }
                        },
                    }
                }
            },
        }
    }
}

/// Applies `url` over the selection through Loro, returning whether it landed.
fn apply_link(doc_state: &Arc<Mutex<DocumentState>>, sync: &InsertLinkSync, url: &str) -> bool {
    let guard = sync.loro_doc.read();
    let Some(ldoc) = guard.as_ref() else {
        return false;
    };
    let cursor = sync.cursor_state.read().clone();
    if set_hyperlink(ldoc, &cursor, url).is_err() {
        return false;
    }
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
