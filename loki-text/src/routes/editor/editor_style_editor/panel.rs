// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level style editor panel component.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_state::StyleDraft;
use super::super::editor_style_catalog::{
    catalog_style_list, get_catalog_style, new_custom_style_id, upsert_catalog_style,
};
use super::conversions::{draft_to_style, style_to_draft};
use super::form::{format_row, name_row, parent_next_row};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Height of the open style editor panel in CSS pixels.
pub(crate) const STYLE_EDITOR_HEIGHT_PX: f32 = 240.0;

/// Renders the inline style catalog editor panel.
///
/// Plain function — no hooks.  Left column shows all catalog styles; right
/// column shows an edit form for the currently selected draft.  Apply commits
/// the draft to the catalog and triggers a full relayout.
pub(crate) fn style_editor_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
) -> Element {
    let draft = match editing_style_draft.read().clone() {
        Some(d) => d,
        None => return rsx! {},
    };

    let ds_list = Arc::clone(&doc_state);
    let ds_new = Arc::clone(&doc_state);
    let ds_apply = Arc::clone(&doc_state);
    let draft_alignment = draft.alignment.clone();
    let biu = [
        ("B", draft.bold),
        ("I", draft.italic),
        ("U", draft.underline),
    ];

    rsx! {
        div {
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: column; flex-shrink: 0; \
                 background: {bg}; border-top: 1px solid {border};",
                h = STYLE_EDITOR_HEIGHT_PX,
                bg = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            // ── Header ────────────────────────────────────────────────────────
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     justify-content: space-between; padding: 0 {p}px; \
                     flex-shrink: 0; height: 28px;",
                    p = tokens::SPACE_4,
                ),
                span {
                    style: format!(
                        "font-family: {ff}; font-size: {fs}px; font-weight: {fw}; color: {fg};",
                        ff = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_LABEL,
                        fw = tokens::FONT_WEIGHT_MEDIUM,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { fl!("ribbon-style-editor-heading") }
                }
                button {
                    style: format!(
                        "background: transparent; border: none; font-size: {fs}px; \
                         color: {fg}; cursor: pointer; padding: {p}px;",
                        fs = tokens::FONT_SIZE_LABEL,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        p = tokens::SPACE_1,
                    ),
                    aria_label: fl!("editor-style-editor-close-aria"),
                    onclick: move |_| editing_style_draft.set(None),
                    "\u{2715}"
                }
            }

            // ── Two-column body ────────────────────────────────────────────────
            div {
                style: "display: flex; flex-direction: row; flex: 1; overflow: hidden;",

                // ── Left: catalog style list ───────────────────────────────────
                { style_list_column(ds_list, ds_new, editing_style_draft, &draft.id) }

                // ── Right: edit form ───────────────────────────────────────────
                div {
                    style: format!(
                        "flex: 1; display: flex; flex-direction: column; \
                         gap: {g}px; padding: {p}px; overflow-y: auto;",
                        g = tokens::SPACE_2,
                        p = tokens::SPACE_3,
                    ),

                    { name_row(editing_style_draft, draft.name.clone()) }
                    { parent_next_row(editing_style_draft, draft.parent.clone(), draft.next.clone()) }
                    { format_row(editing_style_draft, draft_alignment, draft.font_size_str.clone(), biu) }

                    // Apply button
                    div {
                        style: "display: flex; flex-direction: row; gap: 8px; margin-top: auto;",
                        button {
                            style: format!(
                                "padding: {p}px {p2}px; border-radius: {r}px; \
                                 border: 1px solid {border}; cursor: pointer; \
                                 font-family: {ff}; font-size: {fs}px; \
                                 background: {bg}; color: {fg};",
                                p = tokens::SPACE_1,
                                p2 = tokens::SPACE_3,
                                r = tokens::RADIUS_SM,
                                border = tokens::COLOR_TAB_ACTIVE_INDICATOR,
                                ff = tokens::FONT_FAMILY_UI,
                                fs = tokens::FONT_SIZE_BODY,
                                bg = tokens::COLOR_SURFACE_3,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            onclick: move |_| {
                                let v = editing_style_draft.read().clone();
                                if let Some(draft_val) = v {
                                    let style = draft_to_style(&draft_val);
                                    upsert_catalog_style(&ds_apply, style);
                                    let ldoc_guard = loro_doc.read();
                                    if let Some(ldoc) = ldoc_guard.as_ref() {
                                        apply_mutation_and_relayout(&ds_apply, ldoc);
                                    }
                                }
                            },
                            { fl!("ribbon-style-apply-changes") }
                        }
                    }
                }
            }
        }
    }
}

/// Renders the left-hand catalog style list column.
fn style_list_column(
    ds_list: Arc<Mutex<DocumentState>>,
    ds_new: Arc<Mutex<DocumentState>>,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    active_id: &str,
) -> Element {
    let styles = catalog_style_list(&ds_list);
    let active_id = active_id.to_string();

    rsx! {
        div {
            style: format!(
                "width: 160px; min-width: 160px; overflow-y: auto; \
                 border-right: 1px solid {border}; display: flex; \
                 flex-direction: column; gap: 2px; padding: {p}px;",
                border = tokens::COLOR_BORDER_CHROME,
                p = tokens::SPACE_2,
            ),

            {styles.into_iter().map(|(id, display)| {
                let is_active = id == active_id;
                let ds_c = Arc::clone(&ds_list);
                let id_cap = id.clone();
                rsx! {
                    button {
                        key: "{id}",
                        style: format!(
                            "text-align: left; padding: {p}px {p2}px; \
                             border-radius: 3px; border: 1px solid {border}; \
                             cursor: pointer; font-family: {ff}; \
                             font-size: {fs}px; background: {bg}; color: {fg};",
                            p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                            border = if is_active { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                            ff = tokens::FONT_FAMILY_UI,
                            fs = tokens::FONT_SIZE_LABEL,
                            bg = if is_active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        onclick: move |_| {
                            if let Some(s) = get_catalog_style(&ds_c, &id_cap) {
                                editing_style_draft.set(Some(style_to_draft(&s)));
                            }
                        },
                        "{display}"
                    }
                }
            })}

            button {
                style: format!(
                    "padding: {p}px {p2}px; border-radius: 3px; margin-top: {mt}px; \
                     border: 1px solid {border}; cursor: pointer; \
                     font-family: {ff}; font-size: {fs}px; \
                     background: {bg}; color: {fg};",
                    p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                    mt = tokens::SPACE_2,
                    border = tokens::COLOR_BORDER_DEFAULT,
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_LABEL,
                    bg = tokens::COLOR_SURFACE_2,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
                aria_label: fl!("ribbon-style-new-aria"),
                onclick: move |_| {
                    let new_id = new_custom_style_id(&ds_new);
                    editing_style_draft.set(Some(StyleDraft {
                        id: new_id.clone(),
                        name: new_id,
                        is_custom: true,
                        alignment: "Left".to_string(),
                        ..StyleDraft::default()
                    }));
                },
                "+ New"
            }
        }
    }
}
