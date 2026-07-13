// SPDX-License-Identifier: Apache-2.0

//! Paragraph style catalog editor panel for the document editor.
//!
//! `style_editor_panel` renders a two-column panel above the ribbon when
//! `editing_style_draft` is `Some`. The left column lists every catalog style
//! (plus a "+ New" button to create a custom style); the right column
//! ([`form::style_form`]) edits the selected draft, which the Apply button
//! commits to the catalog and relays out.

mod actions;
mod body;
mod char_browser;
mod draft_table;

/// `use_signal` initialiser for the table draft (keeps the draft type private
/// to this module — `editor_inner` only threads the signal through).
pub(super) fn table_draft_none() -> Option<draft_table::TableStyleDraft> {
    None
}
mod char_form;
mod draft;
mod family_inspector;
mod form;
mod form_font;
mod list_browser;
mod page_browser;
mod page_form;
mod page_rename;
mod panel_data;
mod posture;
mod provenance;
mod table_browser;
mod table_form;
mod tree_nav;

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::editor_state::SaveStatus;
use appthere_ui::responsive::Breakpoint;
use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_state::StyleDraft;
use super::editor_style_catalog::{catalog_style_tree, get_catalog_style, reset_style_property};
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use posture::StylePanelPosture;
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
    pub save_message: Signal<Option<SaveStatus>>,
}

/// Renders the inline style catalog editor panel.
///
/// Plain function — no hooks. `font_families` is enumerated once per editor
/// (memoised by the caller) and threaded into the form's font picker. The
/// caller reads the breakpoint (this panel cannot host hooks) and passes it in;
/// [`StylePanelPosture`] maps it to the Compact stacked-sheet layout (§11).
#[allow(clippy::too_many_arguments)]
pub(super) fn style_editor_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    editing_char_style: Signal<Option<String>>,
    editing_char_draft: Signal<Option<StyleDraft>>,
    editing_table_style: Signal<Option<String>>,
    editing_table_draft: Signal<Option<draft_table::TableStyleDraft>>,
    editing_list_style: Signal<Option<String>>,
    editing_page_style: Signal<Option<String>>,
    style_panel_inspect: Signal<bool>,
    breakpoint: Breakpoint,
    font_families: Rc<Vec<String>>,
    sync: StyleEditorSync,
) -> Element {
    let draft = match editing_style_draft.read().clone() {
        Some(d) => d,
        None => return rsx! {},
    };

    let posture = StylePanelPosture::for_breakpoint(breakpoint);
    // When stacked (Compact), the segmented switcher shows one group at a time;
    // when side-by-side (Expanded/Medium), both groups are always visible.
    let inspect = posture.stack && *style_panel_inspect.read();
    let show_edit = !posture.stack || !inspect;
    let show_inspect = !posture.stack || inspect;

    // Family browsers (§9): the char/list style lists + the selected style's
    // read-only rows. Selections live in `editing_char_style` / `editing_list_style`.
    let char_selected = editing_char_style.read().clone();
    let (char_list, char_selected_rows) =
        panel_data::char_data(&doc_state, char_selected.as_deref());
    let table_selected = editing_table_style.read().clone();
    let table_list = table_browser::table_data(&doc_state);
    let ds_table_form = Arc::clone(&doc_state);
    let table_draft = editing_table_draft.read().clone();
    let list_selected = editing_list_style.read().clone();
    let (list_list, list_selected_rows) =
        panel_data::list_data(&doc_state, list_selected.as_deref());
    // Page styles (§9 page family) are derived on demand from the sections.
    let page_selected = editing_page_style.read().clone();
    let (page_list, page_selected_rows) =
        panel_data::page_data(&doc_state, page_selected.as_deref());

    let styles = catalog_style_tree(&doc_state);
    let active_id = draft.id.clone();
    let ds_left = Arc::clone(&doc_state);
    // Editable character form (Spec 05 M6): shown alongside the read-only
    // provenance inspector when a character style is selected.
    let ds_char_form = Arc::clone(&doc_state);
    let char_draft = editing_char_draft.read().clone();
    let char_form_fonts = Rc::clone(&font_families);
    // Editable page form (Spec 05 M6 page family): the selected page style's name
    // + current geometry, for the per-page-style preset buttons.
    let ds_page_form = Arc::clone(&doc_state);
    let page_edit = page_selected
        .as_deref()
        .and_then(|n| panel_data::page_edit_target(&doc_state, n).map(|(l, _)| (n.to_string(), l)));

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
                h = posture.height_px,
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

            // ── Compact-only Edit/Inspect switcher (§11) ───────────────────────
            if posture.stack {
                { body::section_switcher(style_panel_inspect) }
            }

            // ── Body: side-by-side columns at Expanded, stacked sheet at Compact
            div {
                style: format!(
                    "display: flex; flex-direction: {dir}; flex: 1; overflow-{ov};",
                    dir = posture.body_direction(),
                    ov = if posture.stack { "y: auto" } else { ": hidden" },
                ),

                // ── Left: catalog + family lists ───────────────────────────────
                if show_edit {
                    { body::left_column(
                        ds_left,
                        styles,
                        active_id,
                        editing_style_draft,
                        new_style_parent,
                        char_list,
                        char_selected,
                        editing_char_style,
                        editing_char_draft,
                        table_list,
                        table_selected,
                        editing_table_style,
                        editing_table_draft,
                        list_list,
                        list_selected,
                        editing_list_style,
                        page_list,
                        page_selected,
                        editing_page_style,
                        posture,
                    ) }

                    // ── Middle: edit form ──────────────────────────────────────
                    { form::style_form(doc_state, editing_style_draft, draft, font_families, sync) }
                }

                // ── Right: provenance inspector (Spec 05 M2) ───────────────────
                if show_inspect && !display_rows.is_empty() {
                    StyleProvenanceList {
                        rows: display_rows,
                        impact: impact_names,
                        linked,
                        posture,
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

                // ── Right: editable character form (M6) + read-only inspectors ─
                if show_inspect {
                    if let Some(cdraft) = char_draft {
                        { char_form::char_style_form(ds_char_form, editing_char_draft, cdraft, char_form_fonts, sync) }
                    }
                    if let Some(tdraft) = table_draft {
                        { table_form::table_style_form(ds_table_form, editing_table_draft, tdraft, sync) }
                    }
                    if let Some((pname, playout)) = page_edit {
                        { page_form::page_style_form(&ds_page_form, pname, playout, editing_page_style, sync) }
                    }
                    { family_inspector::family_inspector_columns(char_selected_rows, list_selected_rows, page_selected_rows, posture) }
                }
            }
        }
    }
}
