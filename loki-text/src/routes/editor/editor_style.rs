// SPDX-License-Identifier: Apache-2.0

//! Inline paragraph-style picker panel for the document editor.
//!
//! [`style_picker_panel`] returns the conditional element rendered between
//! the canvas area and the ribbon when `is_style_picker_open` is true.  It
//! lists all named paragraph styles from the document's style catalog, plus
//! built-in heading levels derived from the Loro block type.
//!
//! # Layout rationale
//!
//! `position: absolute` is confirmed unsupported in Blitz (dioxus-native).
//! The picker is therefore rendered inline in the editor's flex column, above
//! the ribbon.  When open it claims a fixed slice of vertical space; the canvas
//! area shrinks to compensate (it uses `flex: 1` so Taffy redistributes).
//!
//! # COMPAT(dioxus-native)
//!
//! `white-space: nowrap` and `overflow-x: auto` for horizontal style chips
//! are unconfirmed — using `flex-wrap: wrap` as a fallback.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::set_block_style;
use loki_i18n::fl;

use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use crate::routes::editor::editor_keydown_ctrl::post_mutation_sync;

/// Height of the open style picker panel in CSS pixels.
pub const PICKER_HEIGHT_PX: f32 = 120.0;

/// Returns the list of style names available in the document.
///
/// Combines built-in styles (Default Paragraph Style, Heading 1–6) with
/// all named paragraph styles from the document's style catalog.
/// Duplicates are removed.
pub fn collect_style_names(doc_state: &Arc<Mutex<DocumentState>>) -> Vec<String> {
    let mut names: Vec<String> = vec![
        "Default Paragraph Style".into(),
        "Heading 1".into(),
        "Heading 2".into(),
        "Heading 3".into(),
        "Heading 4".into(),
        "Heading 5".into(),
        "Heading 6".into(),
    ];

    if let Ok(state) = doc_state.lock()
        && let Some(doc) = &state.document
    {
        for (id, style) in &doc.styles.paragraph_styles {
            let name = style
                .display_name
                .clone()
                .unwrap_or_else(|| id.as_str().to_string());
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// Renders the inline style picker panel.
///
/// Plain function — no hooks.  All reactive state is passed in as signals.
#[allow(clippy::too_many_arguments)]
pub fn style_picker_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    mut current_style_name: Signal<String>,
    mut is_style_picker_open: Signal<bool>,
) -> Element {
    let style_names = collect_style_names(&doc_state);
    let current = current_style_name.read().clone();

    rsx! {
        div {
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: column; flex-shrink: 0; \
                 background: {bg}; border-top: 1px solid {border}; \
                 overflow-y: hidden; overflow-x: hidden;",
                h      = PICKER_HEIGHT_PX,
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
                        "font-family: {ff}; font-size: {fs}px; font-weight: {fw}; \
                         color: {fg};",
                        ff = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_LABEL,
                        fw = tokens::FONT_WEIGHT_MEDIUM,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { fl!("ribbon-style-picker-heading") }
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
                    aria_label: "Close style picker",
                    onclick: move |_| { is_style_picker_open.set(false); },
                    "✕"
                }
            }

            // ── Scrollable style chips ────────────────────────────────────────
            // COMPAT(dioxus-native): overflow-x: auto is unconfirmed; using
            // flex-wrap: wrap as a robust fallback.
            div {
                style: format!(
                    "display: flex; flex-direction: row; flex-wrap: wrap; \
                     gap: {g}px; padding: {p}px {p2}px {p}px {p2}px; \
                     overflow-y: auto; flex: 1;",
                    g  = tokens::SPACE_2,
                    p  = tokens::SPACE_1,
                    p2 = tokens::SPACE_4,
                ),

                {style_names.into_iter().map(|name| {
                    let is_active = name == current;
                    let ds = Arc::clone(&doc_state);
                    let n = name.clone();
                    rsx! {
                        button {
                            key: "{name}",
                            style: format!(
                                "padding: {p}px {p2}px; border-radius: 4px; \
                                 border: 1px solid {border}; cursor: pointer; \
                                 font-family: {ff}; font-size: {fs}px; \
                                 background: {bg}; color: {fg}; flex-shrink: 0;",
                                p      = tokens::SPACE_1,
                                p2     = tokens::SPACE_2,
                                border = if is_active {
                                    tokens::COLOR_TAB_ACTIVE_INDICATOR
                                } else {
                                    tokens::COLOR_BORDER_CHROME
                                },
                                ff     = tokens::FONT_FAMILY_UI,
                                fs     = tokens::FONT_SIZE_LABEL,
                                bg     = if is_active {
                                    tokens::COLOR_SURFACE_3
                                } else {
                                    tokens::COLOR_SURFACE_2
                                },
                                fg     = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            aria_label: fl!("ribbon-style-apply-aria", name = name.clone()),
                            onclick: move |_| {
                                let ldoc_guard = loro_doc.read();
                                if let Some(ldoc) = ldoc_guard.as_ref()
                                    && let Some(focus) = cursor_state.read().focus.as_ref()
                                {
                                    let _ = set_block_style(ldoc, focus.paragraph_index, &n);
                                    apply_mutation_and_relayout(&ds, ldoc);
                                }
                                current_style_name.set(name.clone());
                                post_mutation_sync(
                                    &ds,
                                    loro_doc,
                                    cursor_state,
                                    undo_manager,
                                    can_undo,
                                    can_redo,
                                );
                                is_style_picker_open.set(false);
                            },
                            "{name}"
                        }
                    }
                })}
            }
        }
    }
}
