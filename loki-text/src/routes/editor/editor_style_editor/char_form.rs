// SPDX-License-Identifier: Apache-2.0

//! Right-column **character-style** edit form (Spec 05 M6 — the character
//! family, now editable, 4a.3).
//!
//! Character styles carry only run properties, so the form is the character
//! subset of the paragraph form: name, based-on, font family/weight/size, and
//! italic / underline. It reuses the shared inputs ([`field_row`], [`iu_buttons`],
//! [`font_picker`], [`weight_selector`]) — all of which bind a
//! `Signal<Option<StyleDraft>>`, so the character draft rides the same
//! [`StyleDraft`] type (its character fields are a superset). Apply commits a
//! [`CharacterStyle`] to the catalog through Loro and relays out, guarding a
//! cyclic re-parent.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::style::StyleId;
use loki_i18n::fl;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::super::editor_state::StyleDraft;
use super::super::editor_style_catalog::{catalog_snapshot, commit_char_style_to_loro};
use super::StyleEditorSync;
use super::draft::draft_to_char_style;
use super::form::{field_row, iu_buttons};
use super::form_font::{font_picker, weight_selector};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Renders the character-style edit form for the active character draft.
pub(super) fn char_style_form(
    doc_state: Arc<Mutex<DocumentState>>,
    editing_char_draft: Signal<Option<StyleDraft>>,
    draft: StyleDraft,
    font_families: Rc<Vec<String>>,
    sync: StyleEditorSync,
) -> Element {
    let ds_apply = Arc::clone(&doc_state);
    rsx! {
        div {
            style: format!(
                "flex: 1; display: flex; flex-direction: column; gap: {g}px; \
                 padding: {p}px; overflow-y: auto;",
                g = tokens::SPACE_2,
                p = tokens::SPACE_3,
            ),

            div {
                style: format!(
                    "font-family: {ff}; font-size: {fs}px; color: {fg};",
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_XS,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-char-form-heading") }
            }

            { field_row(fl!("editor-style-name-label"), draft.name.clone(), "flex: 1", editing_char_draft, |d, v| d.name = v) }
            { field_row(fl!("editor-style-based-on-label"), draft.parent.clone(), "flex: 1", editing_char_draft, |d, v| d.parent = v) }

            { font_picker(editing_char_draft, draft.font_name.clone(), font_families) }
            { weight_selector(editing_char_draft, draft.font_weight) }

            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 16px; flex-wrap: wrap;",
                { field_row(fl!("editor-style-size-label"), draft.font_size_str.clone(), "width: 48px", editing_char_draft, |d, v| d.font_size_str = v) }
                { iu_buttons(editing_char_draft, draft.italic, draft.underline) }
            }

            { apply_button(ds_apply, editing_char_draft, sync) }
        }
    }
}

/// The Apply button: cycle-guards the based-on, commits the character style, and
/// relays out (mirrors the paragraph form's Apply).
fn apply_button(
    doc_state: Arc<Mutex<DocumentState>>,
    editing_char_draft: Signal<Option<StyleDraft>>,
    sync: StyleEditorSync,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; gap: 8px; margin-top: auto;",
            button {
                style: format!(
                    "padding: {p}px {p2}px; border-radius: {r}px; border: 1px solid {border}; \
                     cursor: pointer; font-family: {ff}; font-size: {fs}px; \
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
                    let Some(draft_val) = editing_char_draft.read().clone() else {
                        return;
                    };
                    // Reject a based-on that would form a cycle (Spec 05 §7).
                    if !draft_val.parent.is_empty() {
                        let child = StyleId::new(&draft_val.id);
                        let new_parent = StyleId::new(&draft_val.parent);
                        let cycles = catalog_snapshot(&doc_state)
                            .is_some_and(|cat| cat.char_reparent_cycles(&child, &new_parent));
                        if cycles {
                            let mut save_message = sync.save_message;
                            save_message.set(Some(fl!("style-reparent-cycle")));
                            return;
                        }
                    }
                    let style = draft_to_char_style(&draft_val);
                    let applied = {
                        let guard = sync.loro_doc.read();
                        if let Some(ldoc) = guard.as_ref() {
                            commit_char_style_to_loro(ldoc, &doc_state, style);
                            apply_mutation_and_relayout(&doc_state, ldoc);
                            true
                        } else {
                            false
                        }
                    };
                    if applied {
                        post_mutation_sync(
                            &doc_state,
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
        }
    }
}
