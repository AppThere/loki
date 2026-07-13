// SPDX-License-Identifier: Apache-2.0

//! Style management actions for the editor panel (Spec 05 M5).
//!
//! [`delete_button`] renders the Delete control for the currently-edited style.
//! It is shown only for user styles (`can_delete`); built-in/default styles are
//! protected (§8). Deleting re-parents the style's children to the grandparent
//! (handled in the model), reports how many were re-parented, and closes the
//! panel. Extracted from `form.rs` to keep that file under the 300-line ceiling.

use std::sync::{Arc, Mutex};

use super::super::editor_state::SaveStatus;
use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::super::editor_state::StyleDraft;
use super::super::editor_style_catalog::{DeleteError, perform_style_delete};
use super::StyleEditorSync;
use crate::editing::state::DocumentState;

/// The Delete control. Returns an empty element when `can_delete` is false, so
/// built-in styles show no affordance.
pub(super) fn delete_button(
    can_delete: bool,
    doc_state: Arc<Mutex<DocumentState>>,
    style_id: String,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    sync: StyleEditorSync,
) -> Element {
    if !can_delete {
        return rsx! {};
    }
    rsx! {
        button {
            style: format!(
                "padding: {p}px {p2}px; border-radius: {r}px; \
                 border: 1px solid {border}; cursor: pointer; \
                 font-family: {ff}; font-size: {fs}px; \
                 background: transparent; color: {fg};",
                p = tokens::SPACE_1,
                p2 = tokens::SPACE_3,
                r = tokens::RADIUS_SM,
                border = tokens::COLOR_BORDER_CHROME,
                ff = tokens::FONT_FAMILY_UI,
                fs = tokens::FONT_SIZE_BODY,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
            ),
            aria_label: fl!("style-delete-aria"),
            onclick: move |_| {
                let result = {
                    let guard = sync.loro_doc.read();
                    guard.as_ref().map(|ldoc| perform_style_delete(ldoc, &doc_state, &style_id))
                };
                let mut save_message = sync.save_message;
                match result {
                    Some(Ok(n)) => {
                        post_mutation_sync(
                            &doc_state,
                            sync.loro_doc,
                            sync.cursor_state,
                            sync.undo_manager,
                            sync.can_undo,
                            sync.can_redo,
                        );
                        save_message.set(Some(SaveStatus::ok(fl!(
                            "style-delete-success",
                            count = n as i64
                        ))));
                        editing_style_draft.set(None); // close the panel; the style is gone
                    }
                    Some(Err(DeleteError::Builtin)) => {
                        save_message.set(Some(SaveStatus::error(fl!("style-delete-builtin"))));
                    }
                    _ => {}
                }
            },
            { fl!("style-delete-label") }
        }
    }
}
