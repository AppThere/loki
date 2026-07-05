// SPDX-License-Identifier: Apache-2.0

//! Save-dialog callbacks extracted from `editor_inner` (an oversized file).
//!
//! Hosts the **Save As** flow (repoints the tab, records recents, navigates
//! to the new path) and the self-contained **Save as Template** flow.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_file_access::{FilePicker, SaveOptions};
use loki_i18n::fl;

use super::editor_save::{export_document_to_token, export_template_to_token};
use crate::editing::state::DocumentState;
use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

/// MIME type used by the Save As flow (Word `.docx`).
const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

/// MIME type used by the "Save as Template" flow (Word `.dotx`).
const DOTX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.template";

/// Builds the Save As callback: picks a destination via the platform save
/// dialog, exports DOCX to it, then repoints the current tab at the new file,
/// records it in recents, and navigates to the new path (the editor reloads
/// it and re-establishes a clean baseline). This is the only way to persist
/// an untitled document.
pub(super) fn use_save_as_callback(
    doc_state: Arc<Mutex<DocumentState>>,
    save_message: Signal<Option<String>>,
    baseline_gen: Signal<u64>,
    path_signal: Signal<String>,
) -> Callback<()> {
    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    let recent_docs = use_context::<Signal<RecentDocuments>>();
    let navigator = use_navigator();
    use_callback(move |_: ()| {
        let doc_state = Arc::clone(&doc_state);
        let mut tabs = tabs;
        let mut recent = recent_docs;
        let mut save_message = save_message;
        let mut baseline_gen = baseline_gen;
        let nav = navigator;
        let cur_path = path_signal.peek().clone();
        let suggested = {
            let stem = display_title_from_path(&cur_path);
            format!("{stem}.docx")
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
                        let new_path = token.serialize();
                        let new_title = display_title_from_path(&new_path);
                        {
                            let mut t = tabs.write();
                            if let Some(tab) = t.iter_mut().find(|tab| tab.path == cur_path) {
                                tab.path = new_path.clone();
                                tab.title = new_title.clone();
                                tab.is_dirty = false;
                            }
                        }
                        recent.write().record(new_path.clone(), new_title);
                        recent.read().save();
                        save_message.set(Some(fl!("editor-save-success")));
                        // Navigate to the saved file; the editor reloads it and
                        // re-establishes a clean baseline.
                        baseline_gen.set(0);
                        nav.push(Route::Editor { path: new_path });
                    }
                    Err(e) => {
                        save_message.set(Some(fl!("editor-save-error", reason = e.to_string())));
                    }
                },
                Ok(None) => { /* user cancelled — no-op */ }
                Err(e) => {
                    save_message.set(Some(fl!("editor-save-error", reason = e.to_string())));
                }
            }
        });
    })
}

/// Builds the "Save as Template" callback: exports the current document as a
/// Word template (`.dotx`) to a picked destination. Unlike Save As it does not
/// repoint the tab — the template is a separate artifact.
pub(super) fn use_save_as_template_callback(
    doc_state: Arc<Mutex<DocumentState>>,
    save_message: Signal<Option<String>>,
    path_signal: Signal<String>,
) -> Callback<()> {
    use_callback(move |_: ()| {
        let doc_state = Arc::clone(&doc_state);
        let mut save_message = save_message;
        let suggested = format!("{}.dotx", display_title_from_path(&path_signal.peek()));
        spawn(async move {
            let picker = FilePicker::new();
            let opts = SaveOptions {
                mime_type: Some(DOTX_MIME.to_string()),
                suggested_name: Some(suggested),
            };
            match picker.pick_file_to_save(opts).await {
                Ok(Some(token)) => {
                    let msg = match export_template_to_token(&token, &doc_state) {
                        Ok(()) => fl!("editor-save-template-success"),
                        Err(e) => fl!("editor-save-error", reason = e.to_string()),
                    };
                    save_message.set(Some(msg));
                }
                Ok(None) => { /* user cancelled — no-op */ }
                Err(e) => {
                    save_message.set(Some(fl!("editor-save-error", reason = e.to_string())));
                }
            }
        });
    })
}
