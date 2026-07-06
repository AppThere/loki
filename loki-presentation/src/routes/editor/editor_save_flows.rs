// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Save and Save As flows for the presentation editor.
//!
//! Extracted from `editor_inner.rs` to keep that file under the 300-line
//! ceiling. Save As picks a destination, exports PPTX, repoints the tab,
//! records recents, and navigates; Save exports to the current token (or
//! routes untitled documents to Save As).

use dioxus::prelude::*;
use loki_file_access::{FileAccessToken, FilePicker, SaveOptions};
use loki_i18n::fl;
use loki_presentation_model::Presentation;

use super::editor_save::export_to_token;
use crate::new_document::is_untitled;
use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

const PPTX_MIME: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

/// Signals the save flows read and write, grouped for the hook call.
pub(super) struct SaveCtx {
    pub doc: Signal<Option<Presentation>>,
    pub path_signal: Signal<String>,
    pub dirty: Signal<bool>,
    pub save_message: Signal<Option<String>>,
}

/// Builds the `(save, save_as)` callbacks. Must be called unconditionally in
/// the component body (it registers hooks).
pub(super) fn use_save_callbacks(ctx: SaveCtx) -> (Callback<()>, Callback<()>) {
    let SaveCtx {
        doc,
        path_signal,
        mut dirty,
        mut save_message,
    } = ctx;
    let navigator = use_navigator();
    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    let recent_docs = use_context::<Signal<RecentDocuments>>();

    // ── Save As ───────────────────────────────────────────────────────────────
    let save_as = use_callback(move |_: ()| {
        let Some(pres) = doc.peek().clone() else {
            return;
        };
        let cur_path = path_signal.peek().clone();
        let suggested = format!("{}.pptx", display_title_from_path(&cur_path));
        let mut tabs = tabs;
        let mut recent = recent_docs;
        let nav = navigator;
        spawn(async move {
            let picker = FilePicker::new();
            let opts = SaveOptions {
                mime_type: Some(PPTX_MIME.to_string()),
                suggested_name: Some(suggested),
            };
            match picker.pick_file_to_save(opts).await {
                Ok(Some(token)) => match export_to_token(&token, &pres) {
                    Ok(()) => {
                        let new_path = token.serialize();
                        let new_title = display_title_from_path(&new_path);
                        {
                            let mut t = tabs.write();
                            if let Some(tab) = t.iter_mut().find(|tb| tb.path == cur_path) {
                                tab.path = new_path.clone();
                                tab.title = new_title.clone();
                                tab.is_dirty = false;
                            }
                        }
                        recent.write().record(new_path.clone(), new_title);
                        recent.read().save();
                        dirty.set(false);
                        save_message.set(Some(fl!("editor-save-success")));
                        nav.push(Route::Editor { path: new_path });
                    }
                    Err(e) => save_message.set(Some(fl!("editor-save-error", reason = e))),
                },
                Ok(None) => {}
                Err(e) => save_message.set(Some(fl!("editor-save-error", reason = e.to_string()))),
            }
        });
    });

    // ── Save ──────────────────────────────────────────────────────────────────
    let save = use_callback(move |_: ()| {
        let cur = path_signal.peek().clone();
        if is_untitled(&cur) {
            save_as.call(());
            return;
        }
        let Some(pres) = doc.peek().clone() else {
            return;
        };
        match FileAccessToken::deserialize(&cur) {
            Ok(token) => match export_to_token(&token, &pres) {
                Ok(()) => {
                    dirty.set(false);
                    save_message.set(Some(fl!("editor-save-success")));
                }
                Err(e) => save_message.set(Some(fl!("editor-save-error", reason = e))),
            },
            Err(e) => save_message.set(Some(fl!("editor-save-error", reason = e.to_string()))),
        }
    });

    (save, save_as)
}
