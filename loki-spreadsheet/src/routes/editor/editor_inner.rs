// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spreadsheet editor inner view.

use appthere_ui::{
    AtIcon, AtRibbon, AtRibbonGroup, AtRibbonIconButton, AtStatusBar, LUCIDE_BOLD, LUCIDE_ITALIC,
    LUCIDE_REDO, LUCIDE_UNDERLINE, LUCIDE_UNDO, RibbonTabDesc, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;
use std::collections::HashSet;

use super::cell_ref::{col_to_label, grid_dimensions};
use super::editor_load::load_document;
use super::editor_mutate::{
    apply_change, mutate_cell, mutate_cell_style, mutate_column_width, sync_undo_redo,
};
use super::editor_path_sync::{
    PathSyncSignals, restore_session, stash_outgoing, sync_path_and_reset,
};
use super::editor_save::save_document;
use super::editor_state::{EditorState, use_editor_state};
use super::formula::{evaluate_cell, format_evaluated_value};
use crate::utils::display_title_from_path;

/// Default rendered column width in CSS px (when the document specifies none).
const DEFAULT_COL_PX: f64 = 100.0;
/// Resize clamps (CSS px).
const MIN_COL_PX: f64 = 24.0;
const MAX_COL_PX: f64 = 800.0;

/// CSS px ↔ points (96 px/in vs 72 pt/in).
fn px_to_pt(px: f64) -> f64 {
    px * 72.0 / 96.0
}
fn pt_to_px(pt: f64) -> f64 {
    pt * 96.0 / 72.0
}

/// In-progress column drag state (CSS px).
#[derive(Clone, Copy, PartialEq)]
struct ColResize {
    col: usize,
    start_x: f64,
    start_px: f64,
    current_px: f64,
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
    // Stashed sessions for inactive tabs — unsaved edits survive tab switches.
    let doc_sessions = use_context::<Signal<crate::sessions::DocSessions>>();

    let EditorState {
        mut workbook_snap,
        mut loro_doc,
        mut undo_manager,
        can_undo,
        can_redo,
        mut selected_cell,
        mut editing_cell,
    } = use_editor_state();

    // Transient save status (success or error) shown as a dismissible banner.
    let mut save_message = use_signal(|| Option::<String>::None);
    // Ribbon chrome state (F6d): active tab + collapse are live signals.
    let mut ribbon_tab = use_signal(|| 0_usize);
    let mut ribbon_collapsed = use_signal(|| false);

    // ── Session restore at mount ─────────────────────────────────────────────
    // Navigating Editor → Home unmounts this component (different routes), so
    // returning to a workbook tab mounts a fresh EditorInner. The matching
    // stash happens in the unmount hook below.
    {
        let mut sessions_at_mount = doc_sessions;
        use_hook(move || {
            let initial_path = path_signal.peek().clone();
            if let Some(session) = sessions_at_mount.write().remove(&initial_path) {
                let mut sig = PathSyncSignals {
                    workbook_snap,
                    loro_doc,
                    undo_manager,
                    can_undo,
                    can_redo,
                    selected_cell,
                    editing_cell,
                };
                restore_session(session, &mut sig);
            }
        });
    }

    // ── Session stash at unmount ─────────────────────────────────────────────
    {
        let tabs_at_drop = tabs;
        let sessions_at_drop = doc_sessions;
        use_drop(move || {
            let old_path = path_signal.peek().clone();
            let mut sig = PathSyncSignals {
                workbook_snap,
                loro_doc,
                undo_manager,
                can_undo,
                can_redo,
                selected_cell,
                editing_cell,
            };
            stash_outgoing(&old_path, tabs_at_drop, sessions_at_drop, &mut sig);
        });
    }

    // ── Synchronous Path Sync & Session Handover ─────────────────────────────
    sync_path_and_reset(
        &path,
        &mut path_signal,
        tabs,
        doc_sessions,
        &mut PathSyncSignals {
            workbook_snap,
            loro_doc,
            undo_manager,
            can_undo,
            can_redo,
            selected_cell,
            editing_cell,
        },
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

    // Grid extent follows the workbook's used range (clamped), so data outside
    // the old fixed A1:J30 window is visible and editable.
    let (num_rows, num_cols) = grid_dimensions(&workbook_snap.read());

    // ── Column widths & drag-to-resize ─────────────────────────────────────────
    let mut col_resize = use_signal(|| Option::<ColResize>::None);

    // Grid zoom (TODO(zoom) resolved, plan 4c.5): the status-bar badge cycles
    // 50–200%; the factor scales the rendered geometry (column widths, row
    // heights, header sizes, fonts) while the *document* stores unzoomed pt —
    // resize commits divide the screen px back out.
    let mut zoom_percent = use_signal(|| 100u32);
    let zf = zoom_percent() as f64 / 100.0;
    let row_h = 26.0 * zf;
    let head_w = 50.0 * zf;
    let font_head = 11.0 * zf;
    let font_cell = 12.0 * zf;

    // The rendered width (screen px) of `col`: the live drag width while
    // resizing, else the document's width (scaled by zoom), else the default.
    let col_px = move |col: usize| -> f64 {
        if let Some(r) = col_resize()
            && r.col == col
        {
            return r.current_px;
        }
        let zf = *zoom_percent.read() as f64 / 100.0;
        workbook_snap
            .read()
            .get_sheet(0)
            .and_then(|s| s.column_width(col as u32))
            .map_or(DEFAULT_COL_PX, pt_to_px)
            * zf
    };

    // Commit a column width (screen px) to the model through Loro, dividing
    // the zoom back out so the document stores the unzoomed width.
    let commit_col_width = move |col: usize, px: f64| {
        let zf = *zoom_percent.peek() as f64 / 100.0;
        let pt = px_to_pt((px / zf).clamp(MIN_COL_PX, MAX_COL_PX));
        apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
            mutate_column_width(ldoc, 0, col as u32, pt)
        });
        sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
    };

    // Auto-fit: width to the widest cell text in the column.
    let auto_fit_col = move |col: usize| {
        let mut max_chars = col_to_label(col).len();
        let wb = workbook_snap.read();
        if let Some(sheet) = wb.get_sheet(0) {
            for row in 0..num_rows {
                if let Some(cell) = sheet.get_cell(row as u32, col as u32) {
                    max_chars = max_chars.max(cell.value.chars().count());
                }
            }
        }
        drop(wb);
        let px = (max_chars as f64 * 7.5 + 16.0).clamp(MIN_COL_PX, MAX_COL_PX);
        // `commit_col_width` takes *screen* px; the estimate is document px.
        commit_col_width(col, px * *zoom_percent.peek() as f64 / 100.0);
    };

    let ref_text = match active_coords {
        Some((r, c)) => format!("{}{}", col_to_label(c), r + 1),
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
                        save_message,
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
                //
                // `tabindex` makes the grid focusable so it can receive key events
                // (clicking a cell focuses this container via the blitz-dom focus
                // patch). Arrow keys / Tab / Enter move the selection, F2 edits,
                // Delete/Backspace clears the cell.
                div {
                    style: "flex: 1; overflow: auto; display: flex; flex-direction: column; background: #FFFFFF; position: relative; outline: none;",
                    tabindex: "0",
                    // COMPAT(dioxus-native): drag-to-resize relies on continuous
                    // onmousemove during a button-held drag, which Blitz may not
                    // deliver yet (needs runtime verification). Double-click
                    // auto-fit uses only click events and is unaffected.
                    onmousemove: move |e| {
                        if let Some(mut r) = col_resize() {
                            let x = e.client_coordinates().x;
                            // Clamp in screen px (the drag space) at the current zoom.
                            let zf = *zoom_percent.peek() as f64 / 100.0;
                            r.current_px = (r.start_px + (x - r.start_x))
                                .clamp(MIN_COL_PX * zf, MAX_COL_PX * zf);
                            col_resize.set(Some(r));
                        }
                    },
                    onmouseup: move |_| {
                        if let Some(r) = col_resize() {
                            commit_col_width(r.col, r.current_px);
                            col_resize.set(None);
                        }
                    },
                    onmouseleave: move |_| {
                        if let Some(r) = col_resize() {
                            commit_col_width(r.col, r.current_px);
                            col_resize.set(None);
                        }
                    },
                    onkeydown: move |e| {
                        if editing_cell.peek().is_some() {
                            return; // the cell input handles keys while editing
                        }
                        let Some((r, c)) = *selected_cell.peek() else {
                            return;
                        };
                        match e.key() {
                            Key::ArrowUp => {
                                e.prevent_default();
                                selected_cell.set(Some((r.saturating_sub(1), c)));
                            }
                            Key::ArrowDown => {
                                e.prevent_default();
                                selected_cell.set(Some(((r + 1).min(num_rows - 1), c)));
                            }
                            Key::ArrowLeft => {
                                e.prevent_default();
                                selected_cell.set(Some((r, c.saturating_sub(1))));
                            }
                            Key::ArrowRight => {
                                e.prevent_default();
                                selected_cell.set(Some((r, (c + 1).min(num_cols - 1))));
                            }
                            Key::Enter => {
                                e.prevent_default();
                                selected_cell.set(Some(((r + 1).min(num_rows - 1), c)));
                            }
                            Key::Tab => {
                                e.prevent_default();
                                let nc = if e.modifiers().shift() {
                                    c.saturating_sub(1)
                                } else {
                                    (c + 1).min(num_cols - 1)
                                };
                                selected_cell.set(Some((r, nc)));
                            }
                            Key::F2 => {
                                e.prevent_default();
                                editing_cell.set(Some((r, c)));
                            }
                            Key::Delete | Key::Backspace => {
                                e.prevent_default();
                                apply_change(loro_doc, workbook_snap, tabs, active_tab_idx, |ldoc| {
                                    mutate_cell(ldoc, 0, r as u32, c as u32, String::new(), None)
                                });
                                sync_undo_redo(loro_doc, undo_manager, can_undo, can_redo);
                            }
                            _ => {}
                        }
                    },

                    // Sticky Header Row
                    div {
                        style: "display: flex; flex-direction: row; position: sticky; top: 0; z-index: 10;",
                        // Corner Header
                        div {
                            style: format!(
                                "width: {head_w}px; height: {row_h}px; background: #E1E1E1; \
                                 border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                                 box-sizing: border-box; flex-shrink: 0;",
                                border = tokens::COLOR_BORDER_DEFAULT,
                            ),
                        }
                        // Column Labels (with a right-edge resize handle)
                        for col_idx in 0..num_cols {
                            div {
                                style: format!(
                                    "width: {w}px; height: {row_h}px; background: {bg}; \
                                     border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                                     box-sizing: border-box; \
                                     display: flex; align-items: center; \
                                     font-size: {font_head}px; font-weight: bold; color: {fg}; \
                                     flex-shrink: 0;",
                                    w = col_px(col_idx),
                                    bg = if is_col_selected(col_idx) { "#CADAFC" } else { "#F0F0F0" },
                                    border = tokens::COLOR_BORDER_DEFAULT,
                                    fg = tokens::COLOR_TEXT_PRIMARY,
                                ),
                                span {
                                    style: "flex: 1; min-width: 0; text-align: center; overflow: hidden;",
                                    "{col_to_label(col_idx)}"
                                }
                                // Drag to resize; double-click to auto-fit.
                                div {
                                    style: "width: 6px; height: 100%; cursor: col-resize; \
                                            flex-shrink: 0; background: rgba(0,0,0,0.06);",
                                    onmousedown: move |e| {
                                        e.stop_propagation();
                                        let start_px = col_px(col_idx);
                                        col_resize.set(Some(ColResize {
                                            col: col_idx,
                                            start_x: e.client_coordinates().x,
                                            start_px,
                                            current_px: start_px,
                                        }));
                                    },
                                    ondoubleclick: move |e| {
                                        e.stop_propagation();
                                        auto_fit_col(col_idx);
                                    },
                                }
                            }
                        }
                    }

                    // Grid Body Rows
                    for row_idx in 0..num_rows {
                        div {
                            style: "display: flex; flex-direction: row;",
                            // Sticky Row Header
                            div {
                                style: format!(
                                    "width: {head_w}px; height: {row_h}px; background: {bg}; \
                                     border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                                     box-sizing: border-box; \
                                     display: flex; align-items: center; justify-content: center; \
                                     font-size: {font_head}px; font-weight: bold; color: {fg}; \
                                     flex-shrink: 0; position: sticky; left: 0; z-index: 5;",
                                    bg = if is_row_selected(row_idx) { "#CADAFC" } else { "#F0F0F0" },
                                    border = tokens::COLOR_BORDER_DEFAULT,
                                    fg = tokens::COLOR_TEXT_PRIMARY,
                                ),
                                "{row_idx + 1}"
                            }

                            // Row Cells
                            for col_idx in 0..num_cols {
                                {
                                    let is_sel = is_cell_selected(row_idx, col_idx);
                                    let is_edit = is_cell_editing(row_idx, col_idx);
                                    let val = get_display_val(row_idx, col_idx);
                                    let fmt = get_cell_format(row_idx, col_idx);
                                    let text_align = match fmt.align {
                                        loki_sheet_model::CellAlign::Center => "center",
                                        loki_sheet_model::CellAlign::Right => "right",
                                        _ => "left",
                                    };
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
                                                "width: {w}px; height: {row_h}px; \
                                                 border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                                                 display: flex; align-items: center; \
                                                 padding: 0 6px; box-sizing: border-box; overflow: hidden; \
                                                 flex-shrink: 0; position: relative; \
                                                 background: {bg_color}; cursor: cell; \
                                                 font-size: {font_cell}px; \
                                                 font-weight: {font_weight}; \
                                                 font-style: {font_style}; \
                                                 text-decoration: {text_decoration}; \
                                                 justify-content: {justify}; \
                                                 outline: {outline}; \
                                                 z-index: {z_index};",
                                                w = col_px(col_idx),
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
                                                    style: format!("width: 100%; height: 100%; border: none; padding: 0; margin: 0; outline: none; font-size: {font_cell}px;"),
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
                                                // The cell's `overflow: hidden` (above) is what
                                                // guarantees text can't paint over neighbouring cells.
                                                // COMPAT(dioxus-native): `text-overflow: ellipsis` and
                                                // `white-space: nowrap` are unconfirmed in Blitz — they
                                                // refine truncation but the cell clip is the hard limit.
                                                span {
                                                    style: format!(
                                                        "display: block; width: 100%; min-width: 0; \
                                                         overflow: hidden; text-overflow: ellipsis; \
                                                         white-space: nowrap; text-align: {text_align};"
                                                    ),
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

            // ── Save status banner (dismissible) ─────────────────────────────
            if let Some(msg) = save_message() {
                div {
                    style: "display: flex; flex-direction: row; justify-content: space-between; \
                            align-items: center; padding: 6px 16px; background: #2A3A4A; \
                            border-bottom: 1px solid #3A4A5A; color: #DCEAF6; font-size: 12px;",
                    span { "{msg}" }
                    button {
                        style: "background: none; border: none; color: #DCEAF6; cursor: pointer; \
                                font-size: 14px; padding: 0 4px;",
                        aria_label: fl!("editor-dismiss-aria"),
                        // Icon-only (×) control: expose a hover tooltip via the
                        // blitz-shell overlay (reads `title`).
                        title:      fl!("editor-dismiss-aria"),
                        onclick: move |_| { save_message.set(None); },
                        "\u{00D7}"
                    }
                }
            }

            {main_content}

            // ── Ribbon (formatting controls) ──────────────────────────────────
            // Only tabs with real content are listed (the loki-text
            // convention) — the earlier Insert/Format/Review/View entries were
            // dead affordances (audit F6d). Collapse is wired for real.
            AtRibbon {
                tabs: vec![
                    RibbonTabDesc { label: fl!("ribbon-tab-home"), is_contextual: false, aria_label: None },
                ],
                active_tab: ribbon_tab(),
                collapsed: ribbon_collapsed(),
                on_toggle_collapse: move |_| {
                    let next = !*ribbon_collapsed.peek();
                    ribbon_collapsed.set(next);
                },
                toggle_aria_label: fl!("ribbon-collapse-aria"),
                on_tab_select: move |idx| ribbon_tab.set(idx),
                tab_content: home_tab
            }

            // ── Status bar ────────────────────────────────────────────────────
            AtStatusBar {
                page_label:         fl!("editor-sheet-label", current = 1i64, total = 1i64),
                word_count_label:   "".to_string(),
                language_label:     fl!("editor-language"),
                zoom_percent:       zoom_percent(),
                collaborator_count: 0,
                collaborator_label: String::new(),
                zoom_aria_label:    fl!("editor-zoom-aria"),
                on_zoom_click:      move |_| {
                    let next = appthere_ui::next_zoom(*zoom_percent.peek());
                    zoom_percent.set(next);
                },
            }
        }
    }
}
