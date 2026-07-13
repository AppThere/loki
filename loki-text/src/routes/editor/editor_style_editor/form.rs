// SPDX-License-Identifier: Apache-2.0

//! Right-column edit form for the style editor panel.
//!
//! `style_form` lays out every editable property of the selected draft (name,
//! based-on / next, font family, weight, size, italic / underline, alignment,
//! indentation and spacing) and an Apply button that commits the draft to the
//! catalog and triggers a relayout.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::super::editor_state::SaveStatus;
use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use loki_doc_model::style::StyleId;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::super::editor_state::StyleDraft;
use super::super::editor_style_catalog::{catalog_snapshot, commit_style_to_loro};
use super::StyleEditorSync;
use super::draft::draft_to_style;
use super::form_font::{font_picker, input_style, label_style, weight_selector};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// A label + text input row whose value is written back to a draft field via
/// `set`. `width_css` controls the input width (e.g. `"flex: 1"`).
pub(super) fn field_row(
    label: String,
    value: String,
    width_css: &str,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    set: impl Fn(&mut StyleDraft, String) + 'static,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 6px;",
            span { style: label_style(), "{label}" }
            input {
                r#type: "text",
                value: "{value}",
                oninput: move |evt| {
                    let v = editing_style_draft.read().clone();
                    if let Some(mut d) = v {
                        set(&mut d, evt.value());
                        editing_style_draft.set(Some(d));
                    }
                },
                style: input_style(width_css),
            }
        }
    }
}

/// Italic / underline toggle buttons (bold is handled by the weight selector).
pub(super) fn iu_buttons(
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    italic: bool,
    underline: bool,
) -> Element {
    let items = [("I", italic, true), ("U", underline, false)];
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 4px;",
            {items.into_iter().map(|(lbl, active, is_italic)| {
                rsx! {
                    button {
                        key: "{lbl}",
                        style: format!(
                            "padding: 2px 8px; border-radius: 3px; border: 1px solid {border}; \
                             cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                             font-style: {fi}; text-decoration: {td}; background: {bg}; color: {fg};",
                            border = if active { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                            ff = tokens::FONT_FAMILY_UI,
                            fs = tokens::FONT_SIZE_LABEL,
                            fi = if is_italic { "italic" } else { "normal" },
                            td = if is_italic { "none" } else { "underline" },
                            bg = if active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        onclick: move |_| {
                            let v = editing_style_draft.read().clone();
                            if let Some(mut d) = v {
                                if is_italic { d.italic = !d.italic; } else { d.underline = !d.underline; }
                                editing_style_draft.set(Some(d));
                            }
                        },
                        "{lbl}"
                    }
                }
            })}
        }
    }
}

/// Alignment selector buttons (Left / Center / Right / Justify).
fn alignment_buttons(
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    current: String,
) -> Element {
    let aligns = [
        ("Left", fl!("editor-style-align-left")),
        ("Center", fl!("editor-style-align-center")),
        ("Right", fl!("editor-style-align-right")),
        ("Justify", fl!("editor-style-align-justify")),
    ];
    rsx! {
        {aligns.into_iter().map(|(val, label)| {
            let is_a = current.as_str() == val;
            rsx! {
                button {
                    key: "{val}",
                    style: format!(
                        "padding: 2px 6px; border-radius: 3px; border: 1px solid {border}; \
                         cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                         background: {bg}; color: {fg};",
                        border = if is_a { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                        ff = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_LABEL,
                        bg = if is_a { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                        fg = tokens::COLOR_TEXT_ON_CHROME,
                    ),
                    onclick: move |_| {
                        let v = editing_style_draft.read().clone();
                        if let Some(mut d) = v {
                            d.alignment = val.to_string();
                            editing_style_draft.set(Some(d));
                        }
                    },
                    "{label}"
                }
            }
        })}
    }
}

/// Renders the right-column edit form for the active draft.
pub(super) fn style_form(
    doc_state: Arc<Mutex<DocumentState>>,
    editing_style_draft: Signal<Option<StyleDraft>>,
    draft: StyleDraft,
    font_families: Rc<Vec<String>>,
    sync: StyleEditorSync,
) -> Element {
    let ds_apply = Arc::clone(&doc_state);
    let ds_delete = Arc::clone(&doc_state);
    let can_delete = draft.is_custom;
    let delete_id = draft.id.clone();
    let align_cur = draft.alignment.clone();
    rsx! {
        div {
            style: format!(
                "flex: 1; display: flex; flex-direction: column; gap: {g}px; \
                 padding: {p}px; overflow-y: auto;",
                g = tokens::SPACE_2,
                p = tokens::SPACE_3,
            ),

            { field_row(fl!("editor-style-name-label"), draft.name.clone(), "flex: 1", editing_style_draft, |d, v| d.name = v) }

            div {
                style: "display: flex; flex-direction: row; gap: 16px;",
                { field_row(fl!("editor-style-based-on-label"), draft.parent.clone(), "flex: 1", editing_style_draft, |d, v| d.parent = v) }
                { field_row(fl!("editor-style-next-style-label"), draft.next.clone(), "flex: 1", editing_style_draft, |d, v| d.next = v) }
            }

            { font_picker(editing_style_draft, draft.font_name.clone(), font_families) }

            { weight_selector(editing_style_draft, draft.font_weight) }

            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 16px; flex-wrap: wrap;",
                { field_row(fl!("editor-style-size-label"), draft.font_size_str.clone(), "width: 48px", editing_style_draft, |d, v| d.font_size_str = v) }
                { iu_buttons(editing_style_draft, draft.italic, draft.underline) }
            }

            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 4px; flex-wrap: wrap;",
                span { style: format!("{} margin-right: 4px;", label_style()), { fl!("editor-style-align-label") } }
                { alignment_buttons(editing_style_draft, align_cur) }
            }

            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 12px; flex-wrap: wrap;",
                span { style: label_style(), { fl!("editor-style-indent-label") } }
                { field_row(fl!("editor-style-indent-left"), draft.indent_start_str.clone(), "width: 48px", editing_style_draft, |d, v| d.indent_start_str = v) }
                { field_row(fl!("editor-style-indent-right"), draft.indent_end_str.clone(), "width: 48px", editing_style_draft, |d, v| d.indent_end_str = v) }
                { field_row(fl!("editor-style-indent-first"), draft.indent_first_str.clone(), "width: 48px", editing_style_draft, |d, v| d.indent_first_str = v) }
                { field_row(fl!("editor-style-indent-hanging"), draft.indent_hanging_str.clone(), "width: 48px", editing_style_draft, |d, v| d.indent_hanging_str = v) }
            }

            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 12px; flex-wrap: wrap;",
                span { style: label_style(), { fl!("editor-style-spacing-label") } }
                { field_row(fl!("editor-style-spacing-before"), draft.space_before_str.clone(), "width: 48px", editing_style_draft, |d, v| d.space_before_str = v) }
                { field_row(fl!("editor-style-spacing-after"), draft.space_after_str.clone(), "width: 48px", editing_style_draft, |d, v| d.space_after_str = v) }
                { field_row(fl!("editor-style-line-spacing"), draft.line_height_str.clone(), "width: 48px", editing_style_draft, |d, v| d.line_height_str = v) }
            }

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
                        let Some(draft_val) = editing_style_draft.read().clone() else {
                            return;
                        };
                        // Guard re-parenting against cycles (Spec 05 §7): reject
                        // an Apply whose based-on would make the tree non-acyclic.
                        if !draft_val.parent.is_empty() {
                            let child = StyleId::new(&draft_val.id);
                            let new_parent = StyleId::new(&draft_val.parent);
                            let cycles = catalog_snapshot(&ds_apply)
                                .is_some_and(|cat| cat.para_reparent_cycles(&child, &new_parent));
                            if cycles {
                                let mut save_message = sync.save_message;
                                save_message.set(Some(SaveStatus::error(fl!("style-reparent-cycle"))));
                                return;
                            }
                        }
                        let style = draft_to_style(&draft_val);
                        // Persist through Loro then re-derive (which reads the
                        // catalog back from the CRDT) so the edit is durable and
                        // undoable. Drop the read guard before the undo refresh.
                        let applied = {
                            let guard = sync.loro_doc.read();
                            if let Some(ldoc) = guard.as_ref() {
                                commit_style_to_loro(ldoc, &ds_apply, style);
                                apply_mutation_and_relayout(&ds_apply, ldoc);
                                true
                            } else {
                                false
                            }
                        };
                        if applied {
                            post_mutation_sync(
                                &ds_apply,
                                sync.loro_doc,
                                sync.cursor_state,
                                sync.undo_manager,
                                sync.can_undo,
                                sync.can_redo,
                            );
                        }
                    },
                    { fl!("ribbon-style-apply-changes") }
                }

                // Delete — user styles only; built-in/default styles are
                // protected (§8). Extracted to keep this file under the ceiling.
                { super::actions::delete_button(can_delete, ds_delete, delete_id, editing_style_draft, sync) }
            }
        }
    }
}
