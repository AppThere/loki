// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Home ribbon tab content builder for the spreadsheet editor.

use appthere_ui::{
    AtIcon, AtRibbonGroup, AtRibbonIconButton, LUCIDE_BOLD, LUCIDE_ITALIC, LUCIDE_REDO,
    LUCIDE_UNDERLINE, LUCIDE_UNDO,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use crate::routes::dioxus_router::Navigator;

use super::document_ops::save_document;
use super::loro_ops::{apply_change, mutate_cell_style, sync_undo_redo};

/// Builds the Home ribbon tab content element.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_home_tab(
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
