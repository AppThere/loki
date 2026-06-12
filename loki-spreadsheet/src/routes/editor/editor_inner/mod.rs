// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spreadsheet editor inner view.

mod cell_ops;
mod document_ops;
mod grid_view;
mod loro_ops;

use appthere_ui::tokens;
use appthere_ui::{
    AtIcon, AtRibbon, AtRibbonGroup, AtRibbonIconButton, AtStatusBar, LUCIDE_BOLD, LUCIDE_ITALIC,
    LUCIDE_REDO, LUCIDE_UNDERLINE, LUCIDE_UNDO, RibbonTabDesc,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_load::load_document;
use super::editor_state::{EditorState, use_editor_state};
use crate::routes::dioxus_router::Navigator;
use crate::utils::display_title_from_path;

use cell_ops::COLS;
use document_ops::{save_document, sync_path_and_reset};
use grid_view::{GridProps, render_formula_bar, render_grid};
use loro_ops::{apply_change, mutate_cell, mutate_cell_style, sync_undo_redo};

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

    let get_cell_format = move |r: usize, c: usize| {
        let wb = workbook_snap.read();
        wb.get_sheet(0)
            .and_then(|s| s.get_cell(r as u32, c as u32))
            .and_then(|cell| cell.style.clone())
            .unwrap_or_default()
    };

    // Formatting Toolbar Active States
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

    let home_tab = build_home_tab(
        is_disabled,
        is_bold_active,
        is_italic_active,
        is_underline_active,
        active_coords,
        path.clone(),
        path_signal,
        workbook_snap,
        tabs,
        active_tab,
        active_tab_idx,
        loro_doc,
        undo_manager,
        can_undo,
        can_redo,
        selected_cell,
        get_cell_format,
        navigator,
    );

    let main_content = match &*document_load.value().read_unchecked() {
        Some((loaded_path, Ok(_))) if loaded_path == &path_signal() => {
            rsx! {
                {render_formula_bar(ref_text, formula_val, EventHandler::new(on_formula_input))}
                {render_grid(GridProps {
                    workbook_snap,
                    loro_doc,
                    undo_manager,
                    can_undo,
                    can_redo,
                    tabs,
                    active_tab_idx,
                    selected_cell,
                    editing_cell,
                    active_coords,
                })}
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

/// Builds the Home ribbon tab content element.
#[allow(clippy::too_many_arguments)]
fn build_home_tab(
    is_disabled: bool,
    is_bold_active: bool,
    is_italic_active: bool,
    is_underline_active: bool,
    active_coords: Option<(usize, usize)>,
    path: String,
    path_signal: Signal<String>,
    mut workbook_snap: Signal<loki_sheet_model::Workbook>,
    tabs: Signal<Vec<crate::tabs::OpenTab>>,
    active_tab: Signal<usize>,
    active_tab_idx: usize,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    selected_cell: Signal<Option<(usize, usize)>>,
    get_cell_format: impl Fn(usize, usize) -> loki_sheet_model::CellStyle + Copy + 'static,
    navigator: Navigator,
) -> Element {
    rsx! {
        AtRibbonGroup {
            label:      None,
            aria_label: "File".to_string(),

            AtRibbonIconButton {
                aria_label:  "Save Document".to_string(),
                is_active:   false,
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
                aria_label:  fl!("ribbon-bold-aria"),
                is_active:   is_bold_active,
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
                aria_label:  fl!("ribbon-italic-aria"),
                is_active:   is_italic_active,
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
                aria_label:  fl!("ribbon-underline-aria"),
                is_active:   is_underline_active,
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
                aria_label:  "Align Left".to_string(),
                is_active:   matches!(active_coords, Some((r, c)) if get_cell_format(r, c).align == loki_sheet_model::CellAlign::Left),
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
                aria_label:  "Align Center".to_string(),
                is_active:   matches!(active_coords, Some((r, c)) if get_cell_format(r, c).align == loki_sheet_model::CellAlign::Center),
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
                aria_label:  "Align Right".to_string(),
                is_active:   matches!(active_coords, Some((r, c)) if get_cell_format(r, c).align == loki_sheet_model::CellAlign::Right),
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
                aria_label:  "Format General".to_string(),
                is_active:   matches!(active_coords, Some((r, c)) if get_cell_format(r, c).num_format == loki_sheet_model::NumberFormat::General),
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
                aria_label:  "Format Currency".to_string(),
                is_active:   matches!(active_coords, Some((r, c)) if get_cell_format(r, c).num_format == loki_sheet_model::NumberFormat::Currency),
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
                aria_label:  "Format Percentage".to_string(),
                is_active:   matches!(active_coords, Some((r, c)) if get_cell_format(r, c).num_format == loki_sheet_model::NumberFormat::Percent),
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
    }
}
