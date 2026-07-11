// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! In-place Loro mutations for the spreadsheet CRDT: cell value/formula,
//! column width, and cell style writes — plus the dispatch (`apply_change`)
//! that commits and re-snapshots after each of these mutations and the
//! undo/redo availability sync. Extracted from `editor_inner.rs` (oversized).

use dioxus::prelude::*;

/// Dispatches changes to Loro, commits, deserializes back, and marks the active tab as dirty
pub(super) fn apply_change<F>(
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut workbook_snap: Signal<loki_sheet_model::Workbook>,
    mut tabs: Signal<Vec<crate::tabs::OpenTab>>,
    active_tab_idx: usize,
    mutate_fn: F,
) where
    F: FnOnce(&loro::LoroDoc) -> Result<(), loro::LoroError>,
{
    let ldoc_guard = loro_doc.read();
    let Some(ldoc) = ldoc_guard.as_ref() else {
        return;
    };

    if let Err(e) = mutate_fn(ldoc) {
        tracing::error!("Failed to mutate LoroDoc: {:?}", e);
        return;
    }

    ldoc.commit();

    match loki_sheet_model::loro_bridge::loro_to_workbook(ldoc) {
        Ok(new_wb) => {
            workbook_snap.set(new_wb);
        }
        Err(e) => {
            tracing::error!("Failed to sync workbook from LoroDoc: {:?}", e);
        }
    }

    if active_tab_idx > 0 {
        let mut tabs_mut = tabs.write();
        if let Some(tab) = tabs_mut.get_mut(active_tab_idx - 1) {
            tab.is_dirty = true;
        }
    }
}

/// Syncs can_undo and can_redo from Loro's UndoManager
pub(super) fn sync_undo_redo(
    loro_doc: Signal<Option<loro::LoroDoc>>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    mut can_undo: Signal<bool>,
    mut can_redo: Signal<bool>,
) {
    if let Some(ldoc) = loro_doc.read().as_ref() {
        ldoc.commit();
    }
    let um_guard = undo_manager.read();
    if let Some(um) = um_guard.as_ref() {
        can_undo.set(um.can_undo());
        can_redo.set(um.can_redo());
    }
}

/// Helper to mutate Loro cells in-place
pub(super) fn mutate_cell(
    ldoc: &loro::LoroDoc,
    sheet_idx: usize,
    row: u32,
    col: u32,
    val: String,
    formula: Option<String>,
) -> Result<(), loro::LoroError> {
    let sheets_list = ldoc.get_list(loki_sheet_model::loro_bridge::KEY_SHEETS);
    let sheet_val = sheets_list
        .get(sheet_idx)
        .ok_or_else(|| loro::LoroError::internal("Sheet not found"))?;
    let sheet_map = sheet_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())
        .ok_or_else(|| loro::LoroError::internal("Sheet is not a map"))?;
    let cells_map = match sheet_map.get("cells") {
        Some(val) => val
            .into_container()
            .ok()
            .and_then(|c| c.into_map().ok())
            .ok_or_else(|| loro::LoroError::internal("Cells container is not a map"))?,
        None => sheet_map.insert_container("cells", loro::LoroMap::new())?,
    };

    let key = format!("{},{}", row, col);
    let cell_map = match cells_map.get(&key) {
        Some(val) => val
            .into_container()
            .ok()
            .and_then(|c| c.into_map().ok())
            .ok_or_else(|| loro::LoroError::internal("Cell container is not a map"))?,
        None => cells_map.insert_container(&key, loro::LoroMap::new())?,
    };

    cell_map.insert("value", val.as_str())?;
    if let Some(f) = formula {
        cell_map.insert("formula", f.as_str())?;
    } else {
        let _ = cell_map.delete("formula");
    }
    Ok(())
}

/// Writes a column width (points) into the Loro sheet's `columns` map.
pub(super) fn mutate_column_width(
    ldoc: &loro::LoroDoc,
    sheet_idx: usize,
    col: u32,
    width_pt: f64,
) -> Result<(), loro::LoroError> {
    let sheets_list = ldoc.get_list(loki_sheet_model::loro_bridge::KEY_SHEETS);
    let sheet_val = sheets_list
        .get(sheet_idx)
        .ok_or_else(|| loro::LoroError::internal("Sheet not found"))?;
    let sheet_map = sheet_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())
        .ok_or_else(|| loro::LoroError::internal("Sheet is not a map"))?;
    let cols_map = match sheet_map.get("columns") {
        Some(val) => val
            .into_container()
            .ok()
            .and_then(|c| c.into_map().ok())
            .ok_or_else(|| loro::LoroError::internal("Columns container is not a map"))?,
        None => sheet_map.insert_container("columns", loro::LoroMap::new())?,
    };
    cols_map.insert(col.to_string().as_str(), width_pt)?;
    Ok(())
}

/// Helper to mutate cell style properties in-place
pub(super) fn mutate_cell_style<F>(
    ldoc: &loro::LoroDoc,
    sheet_idx: usize,
    row: u32,
    col: u32,
    style_fn: F,
) -> Result<(), loro::LoroError>
where
    F: FnOnce(&loro::LoroMap) -> Result<(), loro::LoroError>,
{
    let sheets_list = ldoc.get_list(loki_sheet_model::loro_bridge::KEY_SHEETS);
    let sheet_val = sheets_list
        .get(sheet_idx)
        .ok_or_else(|| loro::LoroError::internal("Sheet not found"))?;
    let sheet_map = sheet_val
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())
        .ok_or_else(|| loro::LoroError::internal("Sheet is not a map"))?;
    let cells_map = match sheet_map.get("cells") {
        Some(val) => val
            .into_container()
            .ok()
            .and_then(|c| c.into_map().ok())
            .ok_or_else(|| loro::LoroError::internal("Cells container is not a map"))?,
        None => sheet_map.insert_container("cells", loro::LoroMap::new())?,
    };

    let key = format!("{},{}", row, col);
    let cell_map = match cells_map.get(&key) {
        Some(val) => val
            .into_container()
            .ok()
            .and_then(|c| c.into_map().ok())
            .ok_or_else(|| loro::LoroError::internal("Cell container is not a map"))?,
        None => cells_map.insert_container(&key, loro::LoroMap::new())?,
    };

    let style_map = match cell_map.get("style") {
        Some(val) => val
            .into_container()
            .ok()
            .and_then(|c| c.into_map().ok())
            .ok_or_else(|| loro::LoroError::internal("Style container is not a map"))?,
        None => {
            let m = cell_map.insert_container("style", loro::LoroMap::new())?;
            m.insert("bold", false)?;
            m.insert("italic", false)?;
            m.insert("underline", false)?;
            m.insert("align", "left")?;
            m.insert("num_format", "general")?;
            m
        }
    };

    style_fn(&style_map)?;
    Ok(())
}
