// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spreadsheet editor inner view.

use appthere_ui::tokens;
use appthere_ui::{
    AtIcon, AtRibbon, AtRibbonGroup, AtRibbonIconButton, AtStatusBar, LUCIDE_BOLD, LUCIDE_ITALIC,
    LUCIDE_REDO, LUCIDE_UNDERLINE, LUCIDE_UNDO, RibbonTabDesc,
};
use dioxus::prelude::*;
use loki_file_access::{FileAccessToken, FilePicker, SaveOptions};
use loki_i18n::fl;
use std::collections::HashSet;

use super::editor_load::{DocumentFormat, detect_format, load_document};
use super::editor_state::{EditorState, use_editor_state};
use crate::routes::Route;
use crate::routes::dioxus_router::Navigator;
use crate::utils::display_title_from_path;

const COLS: &[&str] = &["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];

/// Helper to parse cell reference (e.g. "B2" -> row=1, col=1)
fn parse_cell_ref(s: &str) -> Option<(usize, usize)> {
    let s = s.trim().to_uppercase();
    if s.is_empty() {
        return None;
    }
    let first_char = s.chars().next()?;
    if !first_char.is_ascii_alphabetic() {
        return None;
    }
    let col = (first_char as u32) as i32 - ('A' as u32) as i32;
    if !(0..10).contains(&col) {
        return None;
    }
    let row_str = &s[1..];
    let row = row_str.parse::<usize>().ok()?.checked_sub(1)?;
    if row >= 30 {
        return None;
    }
    Some((row, col as usize))
}

/// Helper to evaluate formulas starting with '=' or cell references in the workbook
fn evaluate_cell(
    row: usize,
    col: usize,
    wb: &loki_sheet_model::Workbook,
    visited: &mut HashSet<(usize, usize)>,
) -> String {
    if visited.contains(&(row, col)) {
        return "#REF!".to_string();
    }
    visited.insert((row, col));

    let sheet = match wb.get_sheet(0) {
        Some(s) => s,
        None => {
            visited.remove(&(row, col));
            return "".to_string();
        }
    };

    let cell = match sheet.get_cell(row as u32, col as u32) {
        Some(c) => c,
        None => {
            visited.remove(&(row, col));
            return "".to_string();
        }
    };

    let Some(formula_raw) = &cell.formula else {
        visited.remove(&(row, col));
        return cell.value.clone();
    };

    let formula = formula_raw.trim().to_uppercase();
    let result = if formula.starts_with("SUM(") && formula.ends_with(')') {
        let range_str = &formula[4..formula.len() - 1];
        if let Some((start, end)) = range_str.split_once(':') {
            if let (Some((r1, c1)), Some((r2, c2))) = (parse_cell_ref(start), parse_cell_ref(end)) {
                let mut sum = 0.0;
                let min_r = r1.min(r2);
                let max_r = r1.max(r2);
                let min_c = c1.min(c2);
                let max_c = c1.max(c2);
                for r in min_r..=max_r {
                    for c in min_c..=max_c {
                        let cell_val_str = evaluate_cell(r, c, wb, visited);
                        if let Ok(num) = cell_val_str.parse::<f64>() {
                            sum += num;
                        }
                    }
                }
                sum.to_string()
            } else {
                "#VALUE!".to_string()
            }
        } else {
            "#VALUE!".to_string()
        }
    } else {
        // Simple expression parser for B2-B3-B4 or B2+B3
        let mut tokens_list = Vec::new();
        let mut current_token = String::new();
        for ch in formula.chars() {
            if ch == '+' || ch == '-' {
                if !current_token.trim().is_empty() {
                    tokens_list.push(current_token.trim().to_string());
                }
                tokens_list.push(ch.to_string());
                current_token = String::new();
            } else {
                current_token.push(ch);
            }
        }
        if !current_token.trim().is_empty() {
            tokens_list.push(current_token.trim().to_string());
        }

        if tokens_list.is_empty() {
            "0".to_string()
        } else {
            let mut total = 0.0;
            let mut next_op = '+';
            let mut first = true;

            for token in tokens_list {
                if token == "+" {
                    next_op = '+';
                } else if token == "-" {
                    next_op = '-';
                } else {
                    let val_f = if let Some((r, c)) = parse_cell_ref(&token) {
                        let cell_val_str = evaluate_cell(r, c, wb, visited);
                        cell_val_str.parse::<f64>().unwrap_or(0.0)
                    } else {
                        token.parse::<f64>().unwrap_or(0.0)
                    };

                    if first {
                        total = val_f;
                        first = false;
                    } else if next_op == '+' {
                        total += val_f;
                    } else {
                        total -= val_f;
                    }
                }
            }
            total.to_string()
        }
    };

    visited.remove(&(row, col));
    result
}

fn format_evaluated_value(val_str: &str, format: &loki_sheet_model::CellStyle) -> String {
    if let Ok(num) = val_str.parse::<f64>() {
        match format.num_format {
            loki_sheet_model::NumberFormat::Currency => format!("${:.2}", num),
            loki_sheet_model::NumberFormat::Percent => format!("{:.1}%", num * 100.0),
            loki_sheet_model::NumberFormat::General => val_str.to_string(),
        }
    } else {
        val_str.to_string()
    }
}

/// Helper to mutate Loro cells in-place
fn mutate_cell(
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

/// Helper to mutate cell style properties in-place
fn mutate_cell_style<F>(
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

/// Dispatches changes to Loro, commits, deserializes back, and marks the active tab as dirty
fn apply_change<F>(
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
fn sync_undo_redo(
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

/// Saves the workbook snapshot to the file target (or picks a target first if untitled)
fn save_document(
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
fn sync_path_and_reset(
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

/// Spreadsheet editor inner component.
#[component]
pub(super) fn EditorInner(path: String) -> Element {
    let navigator = use_navigator();
    let mut path_signal = use_signal(|| path.clone());
    let title = use_memo(move || display_title_from_path(&path_signal.read()));

    let tabs = use_context::<Signal<Vec<crate::tabs::OpenTab>>>();
    let active_tab = use_context::<Signal<usize>>();
    let active_tab_idx = *active_tab.read();

    let EditorState {
        mut workbook_snap,
        mut loro_doc,
        mut undo_manager,
        mut can_undo,
        mut can_redo,
        mut selected_cell,
        mut editing_cell,
    } = use_editor_state();

    // ── Synchronous Path Sync & State Reset ──────────────────────────────────
    sync_path_and_reset(
        &path,
        &mut path_signal,
        &mut workbook_snap,
        &mut loro_doc,
        &mut undo_manager,
        &mut can_undo,
        &mut can_redo,
        &mut selected_cell,
        &mut editing_cell,
    );

    // ── Document load — reactive on path_signal ───────────────────────────────
    let document_load = use_resource(move || {
        let p = path_signal();
        async move {
            let res = load_document(p.clone());
            (p, res)
        }
    });

    // ── Loro bridge: initialise CRDT once the document is loaded ─────────────
    use_effect(move || {
        if let Some((loaded_path, Ok(wb))) = &*document_load.value().read_unchecked()
            && loaded_path == &path_signal()
            && loro_doc().is_none()
        {
            match loki_sheet_model::loro_bridge::workbook_to_loro(wb) {
                Ok(l_doc) => {
                    let um = loro::UndoManager::new(&l_doc);
                    loro_doc.set(Some(l_doc));
                    undo_manager.set(Some(um));
                    workbook_snap.set(wb.clone());
                }
                Err(e) => tracing::warn!("Failed to initialize Loro sync bridge: {}", e),
            }
        }
    });

    let active_coords = selected_cell();

    let ref_text = match active_coords {
        Some((r, c)) => format!("{}{}", COLS[c], r + 1),
        None => "".to_string(),
    };

    let formula_val = match active_coords {
        Some((r, c)) => {
            let wb = workbook_snap.read();
            let cell_opt = wb.get_sheet(0).and_then(|s| s.get_cell(r as u32, c as u32));
            if let Some(cell) = cell_opt {
                if let Some(formula) = &cell.formula {
                    format!("={}", formula)
                } else {
                    cell.value.clone()
                }
            } else {
                "".to_string()
            }
        }
        None => "".to_string(),
    };

    let on_formula_input = move |e: FormEvent| {
        if let Some((r, c)) = selected_cell() {
            let val = e.value();
            let (v, f) = if val.starts_with('=') {
                ("".to_string(), Some(val[1..].to_string()))
            } else {
                (val.clone(), None)
            };
            apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                mutate_cell(ldoc, 0, r as u32, c as u32, v, f)
            });
            sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
        }
    };

    let is_col_selected = move |col_idx: usize| {
        if let Some((_, c)) = active_coords {
            c == col_idx
        } else {
            false
        }
    };

    let is_row_selected = move |row_idx: usize| {
        if let Some((r, _)) = active_coords {
            r == row_idx
        } else {
            false
        }
    };

    let is_cell_selected = move |r: usize, c: usize| {
        if let Some((sel_r, sel_c)) = active_coords {
            sel_r == r && sel_c == c
        } else {
            false
        }
    };

    let is_cell_editing = move |r: usize, c: usize| {
        if let Some((edit_r, edit_c)) = editing_cell() {
            edit_r == r && edit_c == c
        } else {
            false
        }
    };

    let get_display_val = move |r: usize, c: usize| {
        let wb = workbook_snap.read();
        let sheet = wb.get_sheet(0);
        let cell_opt = sheet.and_then(|s| s.get_cell(r as u32, c as u32));
        if let Some(cell) = cell_opt {
            let format = cell.style.clone().unwrap_or_default();
            if cell.formula.is_some() {
                let mut visited = HashSet::new();
                let raw_eval = evaluate_cell(r, c, &wb, &mut visited);
                format_evaluated_value(&raw_eval, &format)
            } else {
                format_evaluated_value(&cell.value, &format)
            }
        } else {
            "".to_string()
        }
    };

    let get_cell_format = move |r: usize, c: usize| {
        let wb = workbook_snap.read();
        wb.get_sheet(0)
            .and_then(|s| s.get_cell(r as u32, c as u32))
            .and_then(|cell| cell.style.clone())
            .unwrap_or_default()
    };

    // Formatting Toolbar Actions
    let is_bold_active = match active_coords {
        Some((r, c)) => get_cell_format(r, c).bold,
        None => false,
    };
    let is_italic_active = match active_coords {
        Some((r, c)) => get_cell_format(r, c).italic,
        None => false,
    };
    let is_underline_active = match active_coords {
        Some((r, c)) => get_cell_format(r, c).underline,
        None => false,
    };

    let is_disabled = loro_doc.read().is_none();

    let home_tab = rsx! {
        AtRibbonGroup {
            label:      None,
            aria_label: "File".to_string(),

            AtRibbonIconButton {
                aria_label: "Save Document".to_string(),
                is_active:  false,
                is_disabled: is_disabled,
                on_click: move |_| {
                    save_document(
                        path.clone(),
                        path_signal,
                        workbook_snap,
                        tabs,
                        active_tab,
                        navigator,
                    );
                },
                span { "Save" }
            }
        }

        AtRibbonGroup {
            label:      None,
            aria_label: fl!("ribbon-group-history"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-undo-aria"),
                is_active:   false,
                is_disabled: is_disabled || !*can_undo.read(),
                on_click: move |_| {
                    {
                        let mut um_guard = undo_manager.write();
                        if let Some(um) = um_guard.as_mut() {
                            let _ = um.undo();
                        }
                    }
                    if let Some(ldoc) = loro_doc.read().as_ref() {
                        ldoc.commit();
                        if let Ok(new_wb) = loki_sheet_model::loro_bridge::loro_to_workbook(ldoc) {
                            workbook_snap.set(new_wb);
                        }
                    }
                    sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_UNDO.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-redo-aria"),
                is_active:   false,
                is_disabled: is_disabled || !*can_redo.read(),
                on_click: move |_| {
                    {
                        let mut um_guard = undo_manager.write();
                        if let Some(um) = um_guard.as_mut() {
                            let _ = um.redo();
                        }
                    }
                    if let Some(ldoc) = loro_doc.read().as_ref() {
                        ldoc.commit();
                        if let Ok(new_wb) = loki_sheet_model::loro_bridge::loro_to_workbook(ldoc) {
                            workbook_snap.set(new_wb);
                        }
                    }
                    sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                },
                AtIcon { path_d: LUCIDE_REDO.to_string() }
            }
        }

        AtRibbonGroup {
            label:      None,
            aria_label: fl!("ribbon-group-inline"),

            AtRibbonIconButton {
                aria_label: fl!("ribbon-bold-aria"),
                is_active:  is_bold_active,
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        let current_bold = get_cell_format(r, c).bold;
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("bold", !current_bold)?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                AtIcon { path_d: LUCIDE_BOLD.to_string() }
            }

            AtRibbonIconButton {
                aria_label: fl!("ribbon-italic-aria"),
                is_active:  is_italic_active,
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        let current_italic = get_cell_format(r, c).italic;
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("italic", !current_italic)?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                AtIcon { path_d: LUCIDE_ITALIC.to_string() }
            }

            AtRibbonIconButton {
                aria_label: fl!("ribbon-underline-aria"),
                is_active:  is_underline_active,
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        let current_underline = get_cell_format(r, c).underline;
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("underline", !current_underline)?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                AtIcon { path_d: LUCIDE_UNDERLINE.to_string() }
            }
        }

        AtRibbonGroup {
            label:      None,
            aria_label: "Alignment".to_string(),

            AtRibbonIconButton {
                aria_label: "Align Left".to_string(),
                is_active:  match active_coords { Some((r, c)) => get_cell_format(r, c).align == loki_sheet_model::CellAlign::Left, _ => false },
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("align", "left")?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                span { "L" }
            }

            AtRibbonIconButton {
                aria_label: "Align Center".to_string(),
                is_active:  match active_coords { Some((r, c)) => get_cell_format(r, c).align == loki_sheet_model::CellAlign::Center, _ => false },
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("align", "center")?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                span { "C" }
            }

            AtRibbonIconButton {
                aria_label: "Align Right".to_string(),
                is_active:  match active_coords { Some((r, c)) => get_cell_format(r, c).align == loki_sheet_model::CellAlign::Right, _ => false },
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("align", "right")?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                span { "R" }
            }
        }

        AtRibbonGroup {
            label:      None,
            aria_label: "Number Formatting".to_string(),

            AtRibbonIconButton {
                aria_label: "Format General".to_string(),
                is_active:  match active_coords { Some((r, c)) => get_cell_format(r, c).num_format == loki_sheet_model::NumberFormat::General, _ => false },
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("num_format", "general")?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                span { "123" }
            }

            AtRibbonIconButton {
                aria_label: "Format Currency".to_string(),
                is_active:  match active_coords { Some((r, c)) => get_cell_format(r, c).num_format == loki_sheet_model::NumberFormat::Currency, _ => false },
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("num_format", "currency")?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                span { "$" }
            }

            AtRibbonIconButton {
                aria_label: "Format Percentage".to_string(),
                is_active:  match active_coords { Some((r, c)) => get_cell_format(r, c).num_format == loki_sheet_model::NumberFormat::Percent, _ => false },
                is_disabled: is_disabled,
                on_click: move |_| {
                    if let Some((r, c)) = selected_cell() {
                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                            mutate_cell_style(ldoc, 0, r as u32, c as u32, |style_map| {
                                style_map.insert("num_format", "percent")?;
                                Ok(())
                            })
                        });
                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                    }
                },
                span { "%" }
            }
        }
    };

    let main_content = match &*document_load.value().read_unchecked() {
        Some((loaded_path, Ok(_))) if loaded_path == &path_signal() => {
            rsx! {
                // ── Formula Bar ──────────────────────────────────────────────────
                div {
                    style: format!(
                        "display: flex; flex-direction: row; align-items: center; \
                         background: {bg}; border-bottom: 1px solid {border}; \
                         padding: 6px 12px; gap: 8px; height: 36px; box-sizing: border-box;",
                        bg = tokens::COLOR_SURFACE_PAGE,
                        border = tokens::COLOR_BORDER_DEFAULT,
                    ),
                    // Reference box
                    div {
                        style: format!(
                            "font-size: {size}px; font-weight: bold; color: {fg}; \
                             min-width: 50px; text-align: center; background: #F0F0F0; \
                             padding: 2px 6px; border-radius: 4px; border: 1px solid {border};",
                            size = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_PRIMARY,
                            border = tokens::COLOR_BORDER_DEFAULT,
                        ),
                        "{ref_text}"
                    }
                    // fx icon
                    span {
                        style: "font-style: italic; color: #888888; font-weight: bold; font-family: serif; font-size: 16px; width: 20px; text-align: center;",
                        "fx"
                    }
                    // Formula input
                    input {
                        style: format!(
                            "flex: 1; border: 1px solid {border}; border-radius: 4px; \
                             padding: 4px 8px; font-size: {size}px; font-family: monospace; \
                             background: {bg}; color: {fg}; outline: none;",
                            border = tokens::COLOR_BORDER_DEFAULT,
                            size = tokens::FONT_SIZE_BODY,
                            bg = tokens::COLOR_SURFACE_PAGE,
                            fg = tokens::COLOR_TEXT_PRIMARY,
                        ),
                        value: "{formula_val}",
                        oninput: on_formula_input,
                    }
                }

                // ── Grid Area ────────────────────────────────────────────────────
                div {
                    style: "flex: 1; overflow: auto; display: flex; flex-direction: column; background: #FFFFFF; position: relative;",

                    // Sticky Header Row
                    div {
                        style: "display: flex; flex-direction: row; position: sticky; top: 0; z-index: 10;",
                        // Corner Header
                        div {
                            style: format!(
                                "width: 50px; height: 26px; background: #E1E1E1; \
                                 border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                                 flex-shrink: 0;",
                                border = tokens::COLOR_BORDER_DEFAULT,
                            ),
                        }
                        // Column Labels
                        for (col_idx, col_name) in COLS.iter().enumerate() {
                            div {
                                style: format!(
                                    "width: 100px; height: 26px; background: {bg}; \
                                     border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                                     display: flex; align-items: center; justify-content: center; \
                                     font-size: 11px; font-weight: bold; color: {fg}; \
                                     flex-shrink: 0;",
                                    bg = if is_col_selected(col_idx) { "#CADAFC" } else { "#F0F0F0" },
                                    border = tokens::COLOR_BORDER_DEFAULT,
                                    fg = tokens::COLOR_TEXT_PRIMARY,
                                ),
                                "{col_name}"
                            }
                        }
                    }

                    // Grid Body Rows
                    for row_idx in 0..30 {
                        div {
                            style: "display: flex; flex-direction: row;",
                            // Sticky Row Header
                            div {
                                style: format!(
                                    "width: 50px; height: 26px; background: {bg}; \
                                     border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                                     display: flex; align-items: center; justify-content: center; \
                                     font-size: 11px; font-weight: bold; color: {fg}; \
                                     flex-shrink: 0; position: sticky; left: 0; z-index: 5;",
                                    bg = if is_row_selected(row_idx) { "#CADAFC" } else { "#F0F0F0" },
                                    border = tokens::COLOR_BORDER_DEFAULT,
                                    fg = tokens::COLOR_TEXT_PRIMARY,
                                ),
                                "{row_idx + 1}"
                            }

                            // Row Cells
                            for col_idx in 0..10 {
                                {
                                    let is_sel = is_cell_selected(row_idx, col_idx);
                                    let is_edit = is_cell_editing(row_idx, col_idx);
                                    let val = get_display_val(row_idx, col_idx);
                                    let fmt = get_cell_format(row_idx, col_idx);
                                    let edit_val = if is_edit {
                                        let wb = workbook_snap.read();
                                        let cell_opt = wb.get_sheet(0).and_then(|s| s.get_cell(row_idx as u32, col_idx as u32));
                                        if let Some(cell) = cell_opt {
                                            if let Some(formula) = &cell.formula {
                                                format!("={}", formula)
                                            } else {
                                                cell.value.clone()
                                            }
                                        } else {
                                            "".to_string()
                                        }
                                    } else {
                                        "".to_string()
                                    };

                                    rsx! {
                                        div {
                                            style: format!(
                                                "width: 100px; height: 26px; \
                                                 border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                                                 display: flex; align-items: center; \
                                                 padding: 0 6px; box-sizing: border-box; \
                                                 flex-shrink: 0; position: relative; \
                                                 background: {bg_color}; cursor: cell; \
                                                 font-size: 12px; \
                                                 font-weight: {font_weight}; \
                                                 font-style: {font_style}; \
                                                 text-decoration: {text_decoration}; \
                                                 justify-content: {justify}; \
                                                 outline: {outline}; \
                                                 z-index: {z_index};",
                                                border = tokens::COLOR_BORDER_DEFAULT,
                                                bg_color = if is_sel { "#E8F0FE" } else { "#FFFFFF" },
                                                font_weight = if fmt.bold { "bold" } else { "normal" },
                                                font_style = if fmt.italic { "italic" } else { "normal" },
                                                text_decoration = if fmt.underline { "underline" } else { "none" },
                                                justify = match fmt.align {
                                                    loki_sheet_model::CellAlign::Center => "center",
                                                    loki_sheet_model::CellAlign::Right => "flex-end",
                                                    _ => "flex-start",
                                                },
                                                outline = if is_sel { "2px solid #3D7EFF" } else { "none" },
                                                z_index = if is_sel { "8" } else { "1" },
                                            ),
                                            onclick: move |_| {
                                                selected_cell.set(Some((row_idx, col_idx)));
                                            },
                                            ondoubleclick: move |_| {
                                                selected_cell.set(Some((row_idx, col_idx)));
                                                editing_cell.set(Some((row_idx, col_idx)));
                                            },

                                            if is_edit {
                                                input {
                                                    style: "width: 100%; height: 100%; border: none; padding: 0; margin: 0; outline: none; font-size: 12px;",
                                                    value: "{edit_val}",
                                                    autofocus: true,
                                                    oninput: move |e| {
                                                        let val = e.value();
                                                        let (v, f) = if val.starts_with('=') {
                                                            ("".to_string(), Some(val[1..].to_string()))
                                                        } else {
                                                            (val.clone(), None)
                                                        };
                                                        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                                                            mutate_cell(ldoc, 0, row_idx as u32, col_idx as u32, v, f)
                                                        });
                                                        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                                                    },
                                                    onblur: move |_| {
                                                        editing_cell.set(None);
                                                    },
                                                    onkeydown: move |e| {
                                                        if e.key() == Key::Enter {
                                                            editing_cell.set(None);
                                                        }
                                                    }
                                                }
                                            } else {
                                                span {
                                                    style: "overflow: hidden; text-overflow: ellipsis; white-space: nowrap; width: 100%;",
                                                    "{val}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Some((loaded_path, Err(e))) if loaded_path == &path_signal() => {
            let msg = e.to_string();
            rsx! {
                super::editor_error_view::EditorErrorView { message: msg }
            }
        }
        _ => {
            rsx! {
                div {
                    style: "display: flex; flex-direction: column; justify-content: center; align-items: center; flex: 1;",
                    span {
                        style: "font-size: 14px; color: #888888;",
                        "Loading document..."
                    }
                }
            }
        }
    };

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex: 1; \
                 overflow: hidden; background: {bg}; font-family: system-ui, sans-serif;",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // ── Title Bar / File Info indicator ──────────────────────────────
            div {
                style: "display: flex; flex-direction: row; justify-content: space-between; align-items: center; \
                        padding: 6px 16px; background: #2E2E2E; border-bottom: 1px solid #3A3A3A;",
                span {
                    style: "font-size: 13px; font-weight: bold; color: #E8E8E8;",
                    "{title}"
                }
                span {
                    style: "font-size: 11px; color: #888888;",
                    "Local File • XLSX / ODS"
                }
            }

            {main_content}

            // ── Ribbon (formatting controls) ──────────────────────────────────
            AtRibbon {
                tabs: vec![
                    RibbonTabDesc { label: fl!("ribbon-tab-home"),   is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-insert"), is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-format"), is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-review"), is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-view"),   is_contextual: false, aria_label: None },
                ],
                active_tab: 0,
                collapsed: false,
                on_toggle_collapse: move |_| {},
                toggle_aria_label: fl!("ribbon-collapse-aria"),
                on_tab_select: move |_idx| {},
                tab_content: home_tab
            }

            // ── Status bar ────────────────────────────────────────────────────
            AtStatusBar {
                page_label:         fl!("editor-sheet-label", current = 1i64, total = 1i64),
                word_count_label:   "".to_string(),
                language_label:     fl!("editor-language"),
                zoom_percent:       100,
                collaborator_count: 0,
                collaborator_label: String::new(),
                zoom_aria_label:    fl!("editor-zoom-aria"),
                on_zoom_click:      |_| {},
            }
        }
    }
}
