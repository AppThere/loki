// SPDX-License-Identifier: Apache-2.0

//! The Publish EPUB 3 dialog (design section 7).
//!
//! A tabbed export with a **standing preflight**: a docked rail at Expanded, a
//! collapsed summary card at the top of the body below 1024 px, and never a
//! modal that appears after you press Publish (design note 27). Each warning
//! links to the dialog that fixes it (note 28), and warnings never block —
//! errors would, which is the distinction that keeps the primary button honest
//! (note 30).

mod audit;
mod body;
mod fonts;
mod preflight;
mod rail;
mod tab_fonts;
mod tabs;

use std::sync::{Arc, Mutex};

use appthere_ui::responsive::use_breakpoint;
use appthere_ui::{
    AtDialogButton, AtDialogShell, AtDialogTabStrip, DialogPosture, DialogWidth, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_publish::{PublishFormat, run_export};
use super::editor_state::SaveStatus;
use crate::editing::state::DocumentState;
use tabs::{PublishOptions, PublishTab};

/// Props for [`PublishEpubDialog`].
#[derive(Clone, Props)]
pub(super) struct PublishEpubDialogProps {
    /// Shared document state.
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    /// `true` while the dialog is open.
    pub(super) open: Signal<bool>,
    /// The document's path, for the suggested output name.
    pub(super) path: Signal<String>,
    /// Status-banner sink for the export result.
    pub(super) save_message: Signal<Option<SaveStatus>>,
    /// Opens the metadata dialog, for the preflight's "fix it there" jump.
    pub(super) open_metadata: Signal<bool>,
}

impl PartialEq for PublishEpubDialogProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.open == other.open
            && self.path == other.path
            && self.save_message == other.save_message
            && self.open_metadata == other.open_metadata
    }
}

/// Renders the Publish EPUB dialog.
// PascalCase for rsx; `#[component]` cannot derive the props comparison.
#[allow(non_snake_case)]
pub(super) fn PublishEpubDialog(props: PublishEpubDialogProps) -> Element {
    let PublishEpubDialogProps {
        doc_state,
        mut open,
        path,
        save_message,
        open_metadata,
    } = props;
    let posture = DialogPosture::for_breakpoint(use_breakpoint());

    let options = use_signal(PublishOptions::default);
    let mut active_tab = use_signal(|| PublishTab::Content);
    let menu_open = use_signal(|| false);
    // Collapsed by default below Expanded, where it is a summary card rather
    // than a rail (note 27).
    let preflight_open = use_signal(|| false);

    let report = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| preflight::run(d)))
        .unwrap_or_default();
    let tab = *active_tab.read();
    let can_publish = report.can_publish();
    let summary = report.summary();
    let ds_export = Arc::clone(&doc_state);

    rsx! {
        AtDialogShell {
            title: fl!("publish-dialog-title"),
            subtitle: rsx! {
                div {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { body::subtitle(&doc_state) }
                }
            },
            close_aria_label: fl!("publish-dialog-close-aria"),
            width: DialogWidth::Wide,
            on_close: move |_| open.set(false),

            tabs: rsx! {
                AtDialogTabStrip {
                    labels: PublishTab::labels(),
                    active: tab.index(),
                    inline_at_medium: PublishTab::INLINE_AT_MEDIUM,
                    menu_open,
                    more_label: fl!("publish-dialog-tabs-more"),
                    on_select: move |idx: usize| active_tab.set(PublishTab::from_index(idx)),
                }
            },

            body: rsx! {
                {
                    body::tab_body(
                        tab,
                        &doc_state,
                        options,
                        posture,
                        &report,
                        preflight_open,
                        open_metadata,
                        open,
                    )
                }
            },

            footer: rsx! {
                span {
                    style: format!(
                        "font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_META,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    {summary}
                }
                div {
                    style: format!(
                        "display: flex; flex-direction: {dir}; gap: {gap}px;",
                        dir = if posture.stack_footer { "column-reverse" } else { "row" },
                        gap = tokens::SPACE_2,
                    ),
                    AtDialogButton {
                        label: fl!("publish-dialog-cancel"),
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| open.set(false),
                    }
                    AtDialogButton {
                        label: fl!("publish-dialog-publish"),
                        primary: true,
                        // Note 30: warnings never reach this. Only an error —
                        // a package that would not open — disables Publish.
                        disabled: !can_publish,
                        min_touch_px: posture.min_touch_px,
                        on_click: move |_| {
                            let cur_path = path.read().clone();
                            // The writer takes no options yet, so the
                            // collected TOC depth is not passed through; the
                            // Output tab's notice says so on screen.
                            run_export(
                                &ds_export,
                                PublishFormat::Epub,
                                &cur_path,
                                save_message,
                            );
                            open.set(false);
                        },
                    }
                }
            },
        }
    }
}
