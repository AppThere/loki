// SPDX-License-Identifier: Apache-2.0

//! The character style picker panel, docked above the ribbon (Format tab).
//!
//! The span-level sibling of `editor_style.rs`'s paragraph style picker, and
//! the ribbon route to the same reusable character styles the span dialog's
//! Character style field applies: chips for every catalog character style,
//! plus **None** to remove the reference (the run falls back to the paragraph
//! level). Applying writes `MARK_CHAR_STYLE_ID` over the selection through
//! `span_dialog::char_style` — one write path, both surfaces.
//!
//! In flow rather than overlaid, for the same displace-don't-cover rationale
//! as the paragraph picker. Plain function — no hooks; all reactive state is
//! passed in as signals.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_style_catalog::char_style_entries;
use super::span_dialog::char_style::{apply_char_style, read_char_style};
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Height of the open picker panel in CSS pixels. Shorter than the paragraph
/// picker's: no search row — character style catalogs are short lists.
pub const CHAR_PICKER_HEIGHT_PX: f32 = 120.0;

/// Renders the character style picker panel.
#[allow(clippy::too_many_arguments)]
pub(super) fn char_style_picker_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    mut is_open: Signal<bool>,
) -> Element {
    let entries = char_style_entries(&doc_state);
    // The reference at the head of the selection — drives the active chip.
    let current = loro_doc
        .read()
        .as_ref()
        .and_then(|ldoc| read_char_style(ldoc, &cursor_state.read()));
    let sync = PickerSync {
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
        is_open,
    };

    rsx! {
        div {
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: column; flex-shrink: 0; \
                 background: {bg}; border-top: 1px solid {border}; \
                 overflow-y: hidden; overflow-x: hidden;",
                h      = CHAR_PICKER_HEIGHT_PX,
                bg     = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            // ── Header row ────────────────────────────────────────────────────
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     justify-content: space-between; padding: 0 {p}px; \
                     flex-shrink: 0; height: 28px;",
                    p = tokens::SPACE_4,
                ),
                span {
                    style: format!(
                        "font-size: {fs}px; font-weight: {fw}; color: {fg};",
                        fs = tokens::FONT_SIZE_LABEL,
                        fw = tokens::FONT_WEIGHT_MEDIUM,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { fl!("ribbon-char-style-picker-heading") }
                }
                button {
                    style: format!(
                        "background: transparent; border: none; \
                         font-size: {fs}px; color: {fg}; cursor: pointer; \
                         padding: {p}px;",
                        fs = tokens::FONT_SIZE_LABEL,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        p  = tokens::SPACE_1,
                    ),
                    aria_label: fl!("ribbon-char-style-picker-close-aria"),
                    onclick: move |_| is_open.set(false),
                    "✕"
                }
            }

            // ── Chips ─────────────────────────────────────────────────────────
            div {
                style: format!(
                    "display: flex; flex-direction: row; flex-wrap: wrap; \
                     gap: {g}px; padding: {p}px {p2}px {p}px {p2}px; \
                     overflow-y: auto; flex: 1; align-content: flex-start;",
                    g  = tokens::SPACE_2,
                    p  = tokens::SPACE_1,
                    p2 = tokens::SPACE_4,
                ),

                if entries.is_empty() {
                    span {
                        style: format!(
                            "font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        { fl!("ribbon-char-style-empty") }
                    }
                } else {
                    button {
                        style: chip(current.is_none()),
                        aria_label: fl!("ribbon-char-style-clear-aria"),
                        onclick: {
                            let ds = Arc::clone(&doc_state);
                            move |_| apply(&ds, sync, None)
                        },
                        { fl!("ribbon-char-style-none") }
                    }
                    {entries.into_iter().map(|(id, display)| {
                        let is_active = current.as_deref() == Some(id.as_str());
                        let ds = Arc::clone(&doc_state);
                        rsx! {
                            button {
                                key: "{id}",
                                style: chip(is_active),
                                aria_label: fl!("ribbon-char-style-apply-aria", name = display.clone()),
                                onclick: move |_| apply(&ds, sync, Some(id.clone())),
                                {display.clone()}
                            }
                        }
                    })}
                }
            }
        }
    }
}

/// The signal bundle every chip applies through. `Copy`, so each chip's
/// `onclick` captures its own copy without clone ceremony.
#[derive(Clone, Copy)]
struct PickerSync {
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    is_open: Signal<bool>,
}

/// Applies (`Some(id)`) or clears (`None`) the character style over the
/// selection, relays out, refreshes undo bookkeeping, and closes the panel.
fn apply(doc_state: &Arc<Mutex<DocumentState>>, sync: PickerSync, next: Option<String>) {
    let PickerSync {
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
        mut is_open,
    } = sync;
    {
        let ldoc_guard = loro_doc.read();
        if let Some(ldoc) = ldoc_guard.as_ref() {
            let before = read_char_style(ldoc, &cursor_state.read());
            if apply_char_style(ldoc, &cursor_state.read(), &before, &next).is_ok() {
                apply_mutation_and_relayout(doc_state, ldoc);
            }
        }
    }
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    is_open.set(false);
}

/// Chip styling shared by the None chip and the style chips.
fn chip(active: bool) -> String {
    format!(
        "padding: {p}px {p2}px; border-radius: 4px; border: 1px solid {border}; \
         cursor: pointer; font-size: {fs}px; background: {bg}; color: {fg}; \
         flex-shrink: 0;",
        p = tokens::SPACE_1,
        p2 = tokens::SPACE_2,
        border = if active {
            tokens::COLOR_TAB_ACTIVE_INDICATOR
        } else {
            tokens::COLOR_BORDER_CHROME
        },
        fs = tokens::FONT_SIZE_BODY,
        bg = if active {
            tokens::COLOR_SURFACE_3
        } else {
            tokens::COLOR_SURFACE_2
        },
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}
