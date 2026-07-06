// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Workbook save flow (XLSX/ODS export to the current or a picked token).
//!
//! Extracted from `editor_inner.rs` (an oversized file).

use dioxus::prelude::*;
use loki_file_access::{FileAccessToken, FilePicker, SaveOptions};
use loki_i18n::fl;

use super::editor_load::{DocumentFormat, detect_format};
use crate::routes::Route;
use crate::routes::dioxus_router::Navigator;
use crate::utils::display_title_from_path;

/// Saves the workbook snapshot to the file target (or picks a target first if untitled)
pub(super) fn save_document(
    _path_prop: String,
    mut path_signal: Signal<String>,
    workbook_snap: Signal<loki_sheet_model::Workbook>,
    mut tabs: Signal<Vec<crate::tabs::OpenTab>>,
    active_tab: Signal<usize>,
    navigator: Navigator,
    mut save_message: Signal<Option<String>>,
) {
    let active_tab_idx = *active_tab.peek();
    let current_path = path_signal.peek().clone();
    let wb = workbook_snap.peek().clone();

    spawn(async move {
        save_message.set(None); // clear any previous status
        let token = if crate::new_document::is_untitled(&current_path) {
            let picker = FilePicker::new();
            let opts = SaveOptions {
                mime_type: Some(
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                ),
                suggested_name: Some("Workbook.xlsx".to_string()),
            };
            match picker.pick_file_to_save(opts).await {
                Ok(Some(t)) => t,
                Ok(None) => return, // user cancelled — not an error
                Err(e) => {
                    save_message.set(Some(fl!("error-file-picker", err = e.to_string())));
                    return;
                }
            }
        } else {
            match FileAccessToken::deserialize(&current_path) {
                Ok(t) => t,
                Err(e) => {
                    save_message.set(Some(fl!("editor-save-error", reason = e.to_string())));
                    return;
                }
            }
        };

        let format = detect_format(&token);
        let mut writer = match token.open_write() {
            Ok(w) => w,
            Err(e) => {
                save_message.set(Some(fl!("editor-save-error", reason = e.to_string())));
                return;
            }
        };

        let res = match format {
            DocumentFormat::Xlsx => loki_ooxml::xlsx::export::XlsxExport::export(&wb, &mut *writer)
                .map_err(|e| e.to_string()),
            DocumentFormat::Ods => {
                loki_odf::OdsExport::export(&wb, &mut *writer).map_err(|e| e.to_string())
            }
            DocumentFormat::Unsupported(ext) => Err(format!("Unsupported format: .{ext}")),
        };

        match res {
            Err(e) => {
                save_message.set(Some(fl!("editor-save-error", reason = e)));
            }
            Ok(()) => {
                let new_path = token.serialize();
                let new_title = display_title_from_path(&new_path);

                path_signal.set(new_path.clone());

                if active_tab_idx > 0 {
                    let mut tabs_mut = tabs.write();
                    if let Some(tab) = tabs_mut.get_mut(active_tab_idx - 1) {
                        tab.path = new_path.clone();
                        tab.title = new_title;
                        tab.is_dirty = false;
                    }
                }

                save_message.set(Some(fl!("editor-save-success")));
                navigator.push(Route::Editor { path: new_path });
            }
        }
    });
}
