// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Grid rendering: formula bar, column/row headers, and cell grid body.

use appthere_ui::tokens;
use dioxus::prelude::*;
use std::collections::HashSet;

use super::cell_ops::{COLS, evaluate_cell, format_evaluated_value};
use super::loro_ops::{apply_change, mutate_cell, sync_undo_redo};

#[allow(clippy::too_many_arguments)]
pub(super) fn render_formula_bar(
    ref_text: String,
    formula_val: String,
    on_formula_input: EventHandler<FormEvent>,
) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; \
                 background: {bg}; border-bottom: 1px solid {border}; \
                 padding: 6px 12px; gap: 8px; height: 36px; box-sizing: border-box;",
                bg = tokens::COLOR_SURFACE_PAGE,
                border = tokens::COLOR_BORDER_DEFAULT,
            ),
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
            span {
                style: "font-style: italic; color: #888888; font-weight: bold; font-family: serif; font-size: 16px; width: 20px; text-align: center;",
                "fx"
            }
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
                oninput: move |e| on_formula_input.call(e),
            }
        }
    }
}

pub(super) struct GridProps {
    pub workbook_snap: Signal<loki_sheet_model::Workbook>,
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    pub can_undo: Signal<bool>,
    pub can_redo: Signal<bool>,
    pub tabs: Signal<Vec<crate::tabs::OpenTab>>,
    pub active_tab_idx: usize,
    pub selected_cell: Signal<Option<(usize, usize)>>,
    pub editing_cell: Signal<Option<(usize, usize)>>,
    pub active_coords: Option<(usize, usize)>,
}

pub(super) fn render_grid(p: GridProps) -> Element {
    let GridProps {
        mut workbook_snap,
        loro_doc,
        undo_manager,
        can_undo,
        can_redo,
        tabs,
        active_tab_idx,
        mut selected_cell,
        mut editing_cell,
        active_coords,
    } = p;

    let is_col_selected = move |col_idx: usize| {
        active_coords.map_or(false, |(_, c)| c == col_idx)
    };

    let is_row_selected = move |row_idx: usize| {
        active_coords.map_or(false, |(r, _)| r == row_idx)
    };

    let is_cell_selected = move |r: usize, c: usize| {
        active_coords.map_or(false, |(sel_r, sel_c)| sel_r == r && sel_c == c)
    };

    let is_cell_editing = move |r: usize, c: usize| {
        editing_cell().map_or(false, |(edit_r, edit_c)| edit_r == r && edit_c == c)
    };

    let get_display_val = move |r: usize, c: usize| {
        let wb = workbook_snap.read();
        let cell_opt = wb.get_sheet(0).and_then(|s| s.get_cell(r as u32, c as u32));
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

    rsx! {
        div {
            style: "flex: 1; overflow: auto; display: flex; flex-direction: column; background: #FFFFFF; position: relative;",

            // Sticky Header Row
            div {
                style: "display: flex; flex-direction: row; position: sticky; top: 0; z-index: 10;",
                div {
                    style: format!(
                        "width: 50px; height: 26px; background: #E1E1E1; \
                         border-right: 1px solid {border}; border-bottom: 1px solid {border}; \
                         flex-shrink: 0;",
                        border = tokens::COLOR_BORDER_DEFAULT,
                    ),
                }
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
