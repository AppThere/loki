// SPDX-License-Identifier: Apache-2.0

//! Inline hyperlink URL panel for the Insert ribbon tab (Spec 04 M4).
//!
//! Rendered above the ribbon (no `position: absolute` in Blitz, mirroring the
//! metadata and style panels). Commits the URL over the current selection / word
//! via [`super::editor_insert::set_hyperlink`] on Apply, or clears it on Remove.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_insert::set_hyperlink;
use super::editor_keydown_ctrl::post_mutation_sync;
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Height of the open link panel in CSS pixels.
pub(super) const LINK_PANEL_HEIGHT_PX: f32 = 96.0;

/// Signals the link panel needs to persist the link through Loro and refresh
/// the undo/dirty state. Grouped to keep the function signature manageable.
#[derive(Clone, Copy)]
pub(super) struct InsertLinkSync {
    /// The document's Loro CRDT handle.
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    /// Cursor state (mirrors the document generation for dirty tracking).
    pub cursor_state: Signal<CursorState>,
    /// Undo manager, refreshed after the mutation.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether undo is available.
    pub can_undo: Signal<bool>,
    /// Whether redo is available.
    pub can_redo: Signal<bool>,
}

/// Renders the hyperlink URL panel when `link_draft` is `Some`.
pub(super) fn insert_link_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    mut link_draft: Signal<Option<String>>,
    sync: InsertLinkSync,
) -> Element {
    let url = match link_draft.read().clone() {
        Some(u) => u,
        None => return rsx! {},
    };
    let ds_apply = Arc::clone(&doc_state);
    let ds_remove = Arc::clone(&doc_state);

    rsx! {
        div {
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: column; flex-shrink: 0; \
                 background: {bg}; border-top: 1px solid {border};",
                h = LINK_PANEL_HEIGHT_PX,
                bg = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            // Header: title + close
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
                    { fl!("ribbon-insert-link-title") }
                }
                button {
                    style: close_button_style(),
                    aria_label: fl!("ribbon-insert-link-cancel"),
                    onclick: move |_| link_draft.set(None),
                    "\u{2715}"
                }
            }

            // URL field
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; gap: {g}px; \
                     padding: {p}px {p2}px;",
                    g = tokens::SPACE_2,
                    p = tokens::SPACE_2,
                    p2 = tokens::SPACE_4,
                ),
                input {
                    r#type: "text",
                    value: "{url}",
                    placeholder: fl!("ribbon-insert-link-placeholder"),
                    aria_label: fl!("ribbon-insert-link-url-aria"),
                    oninput: move |evt| link_draft.set(Some(evt.value())),
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

            // Footer: Remove + Apply
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     justify-content: flex-end; gap: {g}px; padding: {p}px {p2}px; \
                     flex-shrink: 0;",
                    g = tokens::SPACE_2,
                    p = tokens::SPACE_2,
                    p2 = tokens::SPACE_4,
                ),
                button {
                    style: action_button_style(false),
                    onclick: move |_| commit_link(&ds_remove, &sync, link_draft, ""),
                    { fl!("ribbon-insert-link-remove") }
                }
                button {
                    style: action_button_style(true),
                    onclick: move |_| {
                        let value = link_draft.peek().clone().unwrap_or_default();
                        commit_link(&ds_apply, &sync, link_draft, &value);
                    },
                    { fl!("ribbon-insert-link-apply") }
                }
            }
        }
    }
}

/// Applies `value` as a hyperlink over the current range (empty clears it),
/// re-derives the document, refreshes undo/dirty state, and closes the panel.
fn commit_link(
    ds: &Arc<Mutex<DocumentState>>,
    sync: &InsertLinkSync,
    mut link_draft: Signal<Option<String>>,
    value: &str,
) {
    let applied = {
        let guard = sync.loro_doc.read();
        if let Some(ldoc) = guard.as_ref() {
            let _ = set_hyperlink(ldoc, &sync.cursor_state.read(), value);
            apply_mutation_and_relayout(ds, ldoc);
            true
        } else {
            false
        }
    };
    if applied {
        post_mutation_sync(
            ds,
            sync.loro_doc,
            sync.cursor_state,
            sync.undo_manager,
            sync.can_undo,
            sync.can_redo,
        );
    }
    link_draft.set(None);
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
    // Min interactive size: 44×44 logical px (WCAG 2.5.8) — Spec 03 M5 (R-15).
    format!(
        "min-height: {touch}px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; \
         border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; cursor: pointer;",
        touch = tokens::TOUCH_MIN,
        p = tokens::SPACE_3,
        bg = bg,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        ff = tokens::FONT_FAMILY_UI,
        fs = tokens::FONT_SIZE_LABEL,
        fg = fg,
    )
}
