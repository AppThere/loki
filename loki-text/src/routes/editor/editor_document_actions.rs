// SPDX-License-Identifier: Apache-2.0

//! The Write tab Document group's New / Open / Save-a-Copy actions (usage
//! audit §4), as hooks built where the tab/recents context is reachable.
//!
//! Everything here reuses flows that already exist — blank/template tab
//! creation, the platform file picker and the template-detection rule from the
//! Home screen, the recents store, and the Save As exporter. The one new
//! behaviour is **Save a Copy**: `use_save_as_callback` minus the lines that
//! repoint the tab, record recents, and navigate — the copy is a separate
//! artifact and the session stays on the working file.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_file_access::{FilePicker, PickOptions, PickerError, SaveOptions};
use loki_i18n::fl;

use super::editor_save::export_document_to_token;
use super::editor_state::SaveStatus;
use crate::editing::state::DocumentState;
use crate::new_document::{new_blank_tab, new_import_tab, new_template_tab};
use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::routes::home_templates::{MIME_TYPES, template_display_name};
use crate::routes::home_util::{opens_as_detached_copy, push_new_tab, push_or_switch_tab};
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

/// Builds the whole Document-group bundle for the ribbon: the four callbacks
/// this module owns plus the caller-supplied Save As / Save as Template, and
/// the recents store handle the menu snapshots from. One call, so
/// `editor_inner` (baselined over the file ceiling) grows by a line, not ten.
pub(super) fn use_document_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    save_message: Signal<Option<SaveStatus>>,
    path_signal: Signal<String>,
    save_as: Callback<()>,
    save_as_template: Callback<()>,
) -> (
    super::editor_ribbon_document::DocumentGroupActions,
    Signal<RecentDocuments>,
) {
    let recents = use_context::<Signal<RecentDocuments>>();
    let actions = super::editor_ribbon_document::DocumentGroupActions {
        on_new: use_new_document_callback(),
        on_open: use_open_callback(save_message),
        on_open_recent: use_open_recent_callback(),
        save_as,
        save_a_copy: use_save_a_copy_callback(Arc::clone(doc_state), save_message, path_signal),
        save_as_template,
    };
    (actions, recents)
}

/// MIME type of the Save a Copy destination (Word `.docx` — same as Save As).
const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

/// The menu id the New menu uses for the blank document (template ids come
/// from `loki_templates::TEMPLATES` and can never collide with it — see the
/// mechanical check in `editor_ribbon_document`).
pub(super) const BLANK_MENU_ID: &str = "blank";

/// Builds the New-document callback: `BLANK_MENU_ID` opens a blank tab, any
/// other id a bundled-template tab, and navigates to it. Unknown ids are
/// ignored (the menu is built from `TEMPLATES`, so one would be a bug, not a
/// user action).
pub(super) fn use_new_document_callback() -> Callback<String> {
    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    let active_tab = use_context::<Signal<usize>>();
    let navigator = use_navigator();
    use_callback(move |id: String| {
        let tab = if id == BLANK_MENU_ID {
            new_blank_tab()
        } else if loki_templates::TEMPLATES.iter().any(|t| t.id == id) {
            new_template_tab(&id, template_display_name(&id))
        } else {
            return;
        };
        let path = push_new_tab(tabs, active_tab, tab);
        navigator.push(Route::Editor { path });
    })
}

/// Builds the Open callback — the Home screen's open flow, reachable from the
/// ribbon (and Ctrl+O): picker → template files open as fresh detached
/// documents, ordinary files record into recents; either way it navigates.
/// Picker errors surface through the save-status line (the editor has no
/// Home-style error banner).
pub(super) fn use_open_callback(save_message: Signal<Option<SaveStatus>>) -> Callback<()> {
    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    let active_tab = use_context::<Signal<usize>>();
    let recent_docs = use_context::<Signal<RecentDocuments>>();
    let navigator = use_navigator();
    use_callback(move |(): ()| {
        let nav = navigator;
        let tabs = tabs;
        let active_tab = active_tab;
        let mut recent = recent_docs;
        let mut save_message = save_message;
        spawn(async move {
            let picker = FilePicker::new();
            let opts = PickOptions {
                mime_types: MIME_TYPES.iter().map(|s| (*s).to_string()).collect(),
                filter_label: Some(fl!("home-filter-label")),
                multi: false,
            };
            match picker.pick_file_to_open(opts).await {
                Ok(Some(token)) if opens_as_detached_copy(token.display_name()) => {
                    let serialized = token.serialize();
                    let title = display_title_from_path(&serialized);
                    let path = push_new_tab(tabs, active_tab, new_import_tab(&serialized, title));
                    nav.push(Route::Editor { path });
                }
                Ok(Some(token)) => {
                    let path = token.serialize();
                    let title = display_title_from_path(&path);
                    push_or_switch_tab(tabs, active_tab, path.clone());
                    recent.write().record(path.clone(), title);
                    recent.read().save();
                    nav.push(Route::Editor { path });
                }
                Ok(None) => { /* user cancelled — no-op */ }
                Err(PickerError::Platform { .. }) => {
                    save_message.set(Some(SaveStatus::error(fl!("error-picker-not-supported"))));
                }
                Err(e) => {
                    save_message.set(Some(SaveStatus::error(e.to_string())));
                }
            }
        });
    })
}

/// Builds the open-recent callback: switches to (or opens) the tab for a
/// recorded path and navigates. The path comes from the recents menu, so a
/// vanished file surfaces through the editor's load-error path, exactly as it
/// does from the Home screen's list.
pub(super) fn use_open_recent_callback() -> Callback<String> {
    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    let active_tab = use_context::<Signal<usize>>();
    let recent_docs = use_context::<Signal<RecentDocuments>>();
    let navigator = use_navigator();
    use_callback(move |path: String| {
        if path.is_empty() {
            return; // the "no recent documents" placeholder row
        }
        let title = display_title_from_path(&path);
        push_or_switch_tab(tabs, active_tab, path.clone());
        let mut recent = recent_docs;
        recent.write().record(path.clone(), title);
        recent.read().save();
        navigator.push(Route::Editor { path });
    })
}

/// Builds the Save-a-Copy callback: exports the document to a picked
/// destination and reports through the status line — **without** repointing
/// the tab, recording recents, or navigating (contrast `use_save_as_callback`,
/// whose whole point is that the session moves to the new file).
pub(super) fn use_save_a_copy_callback(
    doc_state: Arc<Mutex<DocumentState>>,
    save_message: Signal<Option<SaveStatus>>,
    path_signal: Signal<String>,
) -> Callback<()> {
    use_callback(move |(): ()| {
        let doc_state = Arc::clone(&doc_state);
        let mut save_message = save_message;
        let cur_path = path_signal.peek().clone();
        let suggested = {
            let stem = display_title_from_path(&cur_path);
            fl!("editor-save-copy-suggested-name", stem = stem)
        };
        spawn(async move {
            let picker = FilePicker::new();
            let opts = SaveOptions {
                mime_type: Some(DOCX_MIME.to_string()),
                suggested_name: Some(suggested),
            };
            match picker.pick_file_to_save(opts).await {
                Ok(Some(token)) => match export_document_to_token(&token, &doc_state) {
                    Ok(()) => {
                        save_message.set(Some(SaveStatus::ok(fl!("editor-save-success"))));
                    }
                    Err(e) => {
                        save_message.set(Some(SaveStatus::error(fl!(
                            "editor-save-error",
                            reason = e.to_string()
                        ))));
                    }
                },
                Ok(None) => { /* user cancelled — no-op */ }
                Err(e) => {
                    save_message.set(Some(SaveStatus::error(fl!(
                        "editor-save-error",
                        reason = e.to_string()
                    ))));
                }
            }
        });
    })
}
