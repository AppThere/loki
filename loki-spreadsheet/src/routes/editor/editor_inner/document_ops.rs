// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document-level operations: save and path/state reset.

use dioxus::prelude::*;
use loki_file_access::{FileAccessToken, FilePicker, SaveOptions};

use super::super::editor_load::{DocumentFormat, detect_format};
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
) {
    let active_tab_idx = *active_tab.peek();
    let current_path = path_signal.peek().clone();
    let wb = workbook_snap.peek().clone();

    spawn(async move {
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
                Ok(None) => return,
                Err(e) => {
                    tracing::error!("Failed to pick save path: {:?}", e);
                    return;
                }
            }
        } else {
            match FileAccessToken::deserialize(&current_path) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to deserialize path token: {:?}", e);
                    return;
                }
            }
        };

        let format = detect_format(&token);
        match token.open_write() {
            Ok(mut writer) => {
                let res = match format {
                    DocumentFormat::Xlsx => {
                        loki_ooxml::xlsx::export::XlsxExport::export(&wb, &mut *writer)
                            .map_err(|e| e.to_string())
                    }
                    DocumentFormat::Ods => {
                        loki_odf::OdsExport::export(&wb, &mut *writer).map_err(|e| e.to_string())
                    }
                    DocumentFormat::Unsupported(ext) => {
                        Err(format!("Unsupported format: .{}", ext))
                    }
                };

                if let Err(e) = res {
                    tracing::error!("Failed to export workbook: {:?}", e);
                } else {
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

                    navigator.push(Route::Editor { path: new_path });
                }
            }
            Err(e) => {
                tracing::error!("Failed to open file for writing: {:?}", e);
            }
        }
    });
}

/// Reset per-document state when switching paths reactively
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_path_and_reset(
    path: &str,
    path_signal: &mut Signal<String>,
    workbook_snap: &mut Signal<loki_sheet_model::Workbook>,
    loro_doc: &mut Signal<Option<loro::LoroDoc>>,
    undo_manager: &mut Signal<Option<loro::UndoManager>>,
    can_undo: &mut Signal<bool>,
    can_redo: &mut Signal<bool>,
    selected_cell: &mut Signal<Option<(usize, usize)>>,
    editing_cell: &mut Signal<Option<(usize, usize)>>,
) {
    let current = path_signal.peek().clone();
    if current == path {
        return;
    }
    tracing::debug!(
        "EditorInner: path changed from {} to {} -> resetting state",
        current,
        path
    );
    path_signal.set(path.to_owned());
    workbook_snap.set(loki_sheet_model::Workbook::new());
    loro_doc.set(None);
    undo_manager.set(None);
    can_undo.set(false);
    can_redo.set(false);
    selected_cell.set(Some((0, 0)));
    editing_cell.set(None);
}
