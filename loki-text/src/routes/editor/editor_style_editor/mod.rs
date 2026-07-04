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
mod draft;
mod family_inspector;
mod form;
mod form_font;
mod list_browser;
mod panel_data;
mod posture;
mod provenance;

use std::rc::Rc;
use std::sync::{Arc, Mutex};

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
    pub save_message: Signal<Option<String>>,
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
    editing_list_style: Signal<Option<String>>,
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
    let list_selected = editing_list_style.read().clone();
    let (list_list, list_selected_rows) =
        panel_data::list_data(&doc_state, list_selected.as_deref());

    let styles = catalog_style_tree(&doc_state);
    let active_id = draft.id.clone();
    let ds_left = Arc::clone(&doc_state);

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
                        list_list,
                        list_selected,
                        editing_list_style,
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

                // ── Right: character + list inspectors (read-only; §9) ─────────
                if show_inspect {
                    { family_inspector::family_inspector_columns(char_selected_rows, list_selected_rows, posture) }
                }
            }
        }
    }
}
