// SPDX-License-Identifier: Apache-2.0

//! Inline metadata editor panel UI for the Publish ribbon tab.
//!
//! Renders the Dublin Core fields defined in `editor_metadata` as a scrollable
//! form above the ribbon, committing edits via
//! [`super::editor_metadata::apply_meta_draft`] on Save.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_metadata::{MetaDraft, MetaField, apply_meta_draft};
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Height of the open metadata panel in CSS pixels.
pub(super) const METADATA_PANEL_HEIGHT_PX: f32 = 280.0;

/// Signals the metadata panel needs to persist edits through Loro and refresh
/// the undo/dirty state. Grouped to keep the function signature manageable.
#[derive(Clone, Copy)]
pub(super) struct MetaPanelSync {
    /// The document's Loro CRDT handle.
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    /// Cursor state (mirrors the document generation for dirty tracking).
    pub cursor_state: Signal<CursorState>,
    /// Undo manager, refreshed after the metadata mutation.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether undo is available.
    pub can_undo: Signal<bool>,
    /// Whether redo is available.
    pub can_redo: Signal<bool>,
}

/// Renders the metadata editor panel when `editing_metadata` is `Some`.
pub(super) fn metadata_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    mut editing_metadata: Signal<Option<MetaDraft>>,
    mut save_message: Signal<Option<String>>,
    sync: MetaPanelSync,
) -> Element {
    let draft = match editing_metadata.read().clone() {
        Some(d) => d,
        None => return rsx! {},
    };
    let ds_apply = Arc::clone(&doc_state);

    rsx! {
        div {
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: column; flex-shrink: 0; \
                 background: {bg}; border-top: 1px solid {border};",
                h = METADATA_PANEL_HEIGHT_PX,
                bg = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            // Header
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
                    { fl!("metadata-panel-title") }
                }
                button {
                    style: close_button_style(),
                    aria_label: fl!("metadata-cancel"),
                    onclick: move |_| editing_metadata.set(None),
                    "\u{2715}"
                }
            }

            // Scrollable form
            div {
                style: format!(
                    "flex: 1; overflow-y: auto; display: flex; flex-direction: column; \
                     gap: {g}px; padding: {p}px {p2}px;",
                    g = tokens::SPACE_2,
                    p = tokens::SPACE_2,
                    p2 = tokens::SPACE_4,
                ),
                for (idx, (field, value)) in draft.values.iter().enumerate() {
                    {field_row(*field, value.clone(), idx, editing_metadata)}
                }
            }

            // Footer
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     justify-content: flex-end; gap: {g}px; padding: {p}px {p2}px; \
                     flex-shrink: 0; border-top: 1px solid {border};",
                    g = tokens::SPACE_2,
                    p = tokens::SPACE_2,
                    p2 = tokens::SPACE_4,
                    border = tokens::COLOR_BORDER_CHROME,
                ),
                button {
                    style: action_button_style(false),
                    onclick: move |_| editing_metadata.set(None),
                    { fl!("metadata-cancel") }
                }
                button {
                    style: action_button_style(true),
                    onclick: move |_| {
                        if let Some(d) = editing_metadata.peek().clone() {
                            // Persist through Loro, then re-derive the document
                            // (which reads metadata back from the CRDT) so the
                            // change is durable and undoable.
                            let applied = {
                                let guard = sync.loro_doc.read();
                                if let Some(ldoc) = guard.as_ref() {
                                    apply_meta_draft(ldoc, &ds_apply, &d);
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
                                save_message.set(Some(fl!("metadata-saved")));
                            }
                        }
                        editing_metadata.set(None);
                    },
                    { fl!("metadata-save") }
                }
            }
        }
    }
}

/// Renders one labelled text field bound to entry `idx` of the draft.
fn field_row(
    field: MetaField,
    value: String,
    idx: usize,
    mut editing_metadata: Signal<Option<MetaDraft>>,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
            span {
                style: format!(
                    "font-family: {ff}; font-size: {fs}px; color: {fg}; \
                     min-width: 140px; max-width: 140px;",
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { field.label() }
            }
            input {
                r#type: "text",
                value: "{value}",
                oninput: move |evt| {
                    let current = editing_metadata.peek().clone();
                    if let Some(mut d) = current {
                        if let Some(slot) = d.values.get_mut(idx) {
                            slot.1 = evt.value();
                        }
                        editing_metadata.set(Some(d));
                    }
                },
                style: format!(
                    "flex: 1; height: 24px; padding: 0 {p}px; background: {bg}; \
                     border: 1px solid {border}; border-radius: {r}px; \
                     font-family: {ff}; font-size: {fs}px; color: {fg}; \
                     box-sizing: border-box;",
                    p = tokens::SPACE_2,
                    bg = tokens::COLOR_SURFACE_2,
                    border = tokens::COLOR_BORDER_DEFAULT,
                    r = tokens::RADIUS_SM,
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_BODY,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
            }
        }
    }
}

fn close_button_style() -> String {
    format!(
        "background: transparent; border: none; font-size: {fs}px; \
         color: {fg}; cursor: pointer; padding: {p}px;",
        fs = tokens::FONT_SIZE_LABEL,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
        p = tokens::SPACE_1,
    )
}

fn action_button_style(primary: bool) -> String {
    let (bg, fg) = if primary {
        (tokens::COLOR_TAB_ACTIVE_BG, tokens::COLOR_TEXT_ACCENT)
    } else {
        (tokens::COLOR_SURFACE_3, tokens::COLOR_TEXT_ON_CHROME)
    };
    format!(
        "min-height: 28px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; \
         border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; cursor: pointer;",
        p = tokens::SPACE_3,
        bg = bg,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        ff = tokens::FONT_FAMILY_UI,
        fs = tokens::FONT_SIZE_LABEL,
        fg = fg,
    )
}
