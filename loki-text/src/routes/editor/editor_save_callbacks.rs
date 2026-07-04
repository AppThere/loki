// SPDX-License-Identifier: Apache-2.0

//! Save-dialog callbacks extracted from `editor_inner` (an oversized file).
//!
//! Currently hosts the **Save as Template** flow, which is self-contained (it
//! does not repoint the tab or navigate). Save As stays in `editor_inner`
//! because it also touches tab/recents/navigation state.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_file_access::{FilePicker, SaveOptions};
use loki_i18n::fl;

use super::editor_save::export_template_to_token;
use crate::editing::state::DocumentState;
use crate::utils::display_title_from_path;

/// MIME type used by the "Save as Template" flow (Word `.dotx`).
const DOTX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.template";

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
