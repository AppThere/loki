// SPDX-License-Identifier: Apache-2.0

//! Paragraph style catalog editor panel for the document editor.
//!
//! `style_editor_panel` renders a two-column panel above the ribbon when
//! `editing_style_draft` is `Some`. The left column lists every catalog style
//! (plus a "+ New" button to create a custom style); the right column
//! ([`form::style_form`]) edits the selected draft, which the Apply button
//! commits to the catalog and relays out.

mod actions;
mod char_browser;
mod draft;
mod form;
mod form_font;
mod panel_data;
mod provenance;

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_state::StyleDraft;
use super::editor_style_catalog::{
    catalog_style_tree, get_catalog_style, new_custom_style_id, reset_style_property,
};
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use provenance::StyleProvenanceList;

pub(super) use draft::style_to_draft;

/// Height of the open style editor panel in CSS pixels.
pub(super) const STYLE_EDITOR_HEIGHT_PX: f32 = 360.0;

/// Signals the style editor needs to persist edits through Loro and refresh the
/// undo/redo state. Grouped to keep the function signature manageable (mirrors
/// `editor_metadata_panel::MetaPanelSync`).
#[derive(Clone, Copy)]
pub(super) struct StyleEditorSync {
    /// The document's Loro CRDT handle.
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    /// Cursor state (mirrors the document generation for dirty tracking).
    pub cursor_state: Signal<CursorState>,
    /// Undo manager, refreshed after the style mutation.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether undo is available.
    pub can_undo: Signal<bool>,
    /// Whether redo is available.
    pub can_redo: Signal<bool>,
    /// Status-banner sink for feedback (e.g. a rejected cyclic re-parent).
    pub save_message: Signal<Option<String>>,
}

/// Renders the inline style catalog editor panel.
///
/// Plain function — no hooks. `font_families` is enumerated once per editor
/// (memoised by the caller) and threaded into the form's font picker.
pub(super) fn style_editor_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    editing_char_style: Signal<Option<String>>,
    font_families: Rc<Vec<String>>,
    sync: StyleEditorSync,
) -> Element {
    let draft = match editing_style_draft.read().clone() {
        Some(d) => d,
        None => return rsx! {},
    };

    // Character-family browser (§9): the char-style list + the selected style's
    // read-only rows. The selection lives in `editing_char_style`.
    let char_selected = editing_char_style.read().clone();
    let (char_list, char_selected_rows) =
        panel_data::char_data(&doc_state, char_selected.as_deref());

    let styles = catalog_style_tree(&doc_state);
    let active_id = draft.id.clone();
    let ds_list = Arc::clone(&doc_state);
    let ds_new = Arc::clone(&doc_state);

    // Everything the provenance column renders (staged rows, impact preview,
    // new-style parent default, linked character-style rows) — see `panel_data`.
    let panel_data::InspectorData {
        display_rows,
        impact_names,
        new_style_parent,
        linked,
    } = panel_data::inspector_data(&doc_state, &draft);
    // Handles for the reset-to-inherited action on locally-set inspector rows.
    let ds_reset = Arc::clone(&doc_state);
    let reset_id = draft.id.clone();
    // Handle for the jump-to-ancestor link on inherited inspector rows.
    let ds_jump = Arc::clone(&doc_state);

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
                div {
                    style: format!(
                        "width: 160px; min-width: 160px; overflow-y: auto; \
                         border-right: 1px solid {border}; display: flex; \
                         flex-direction: column; gap: 2px; padding: {p}px;",
                        border = tokens::COLOR_BORDER_CHROME,
                        p = tokens::SPACE_2,
                    ),

                    // Inheritance-tree picker: styles indented by depth so the
                    // parent → child hierarchy is visible (Spec 05 §7).
                    {styles.into_iter().map(|(id, display, depth)| {
                        let is_active = id == active_id;
                        let ds_c = Arc::clone(&ds_list);
                        let id_cap = id.clone();
                        rsx! {
                            button {
                                key: "{id}",
                                style: format!(
                                    "text-align: left; padding: {p}px {p2}px; \
                                     padding-left: {indent}px; \
                                     border-radius: 3px; border: 1px solid {border}; \
                                     cursor: pointer; font-family: {ff}; \
                                     font-size: {fs}px; background: {bg}; color: {fg};",
                                    p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                                    indent = tokens::SPACE_2 + depth as f32 * tokens::SPACE_3,
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
                                // Inherit from the current style; leave props
                                // empty so every value shows as inherited.
                                parent: new_style_parent.clone(),
                                ..StyleDraft::default()
                            }));
                        },
                        { fl!("editor-style-new") }
                    }

                    // ── Character styles (§9 character family) ─────────────────
                    { char_browser::char_list_section(char_list, char_selected, editing_char_style) }
                }

                // ── Middle: edit form ──────────────────────────────────────────
                { form::style_form(doc_state, editing_style_draft, draft, font_families, sync) }

                // ── Right: provenance inspector (Spec 05 M2) ───────────────────
                if !display_rows.is_empty() {
                    StyleProvenanceList {
                        rows: display_rows,
                        impact: impact_names,
                        linked,
                        on_reset: move |property| {
                            {
                                let ldoc_guard = sync.loro_doc.read();
                                let Some(ldoc) = ldoc_guard.as_ref() else {
                                    return;
                                };
                                reset_style_property(ldoc, &ds_reset, &reset_id, property);
                                apply_mutation_and_relayout(&ds_reset, ldoc);
                            }
                            post_mutation_sync(
                                &ds_reset,
                                sync.loro_doc,
                                sync.cursor_state,
                                sync.undo_manager,
                                sync.can_undo,
                                sync.can_redo,
                            );
                            // Re-derive the draft so the form + inspector reflect
                            // the reset (the cleared property now shows inherited).
                            if let Some(updated) = get_catalog_style(&ds_reset, &reset_id) {
                                editing_style_draft.set(Some(style_to_draft(&updated)));
                            }
                        },
                        on_jump: move |ancestor: loki_doc_model::style::StyleId| {
                            // Open the ancestor that sets an inherited property, so
                            // the user can change it there for all dependents.
                            if let Some(s) = get_catalog_style(&ds_jump, ancestor.as_str()) {
                                editing_style_draft.set(Some(style_to_draft(&s)));
                            }
                        },
                    }
                }

                // ── Right: character inspector (read-only; §9) ─────────────────
                if let Some((name, rows)) = char_selected_rows {
                    div {
                        style: format!(
                            "width: 220px; min-width: 220px; overflow-y: auto; \
                             border-left: 1px solid {border}; padding: {p}px;",
                            border = tokens::COLOR_BORDER_CHROME,
                            p = tokens::SPACE_3,
                        ),
                        provenance::CharRowsSection { heading: name, rows }
                    }
                }
            }
        }
    }
}
