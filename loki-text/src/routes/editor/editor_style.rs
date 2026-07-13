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
use loki_doc_model::{set_block_style, set_block_type_heading, set_block_type_para};
use loki_i18n::fl;

use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};
use crate::routes::editor::editor_keydown_ctrl::post_mutation_sync;

#[path = "editor_style_data.rs"]
mod data;
pub use data::collect_style_names;
use data::style_preview_font;

/// Height of the open style picker panel in CSS pixels.
pub const PICKER_HEIGHT_PX: f32 = 160.0;

/// Renders the inline style picker panel.
///
/// Plain function — no hooks.  All reactive state is passed in as signals.
/// `current_style_name` is a plain `String` computed inline in `EditorInner`'s
/// render body, ensuring it is always current without a post-render effect.
/// `style_search_query` is cleared on close so the next open starts fresh.
#[allow(clippy::too_many_arguments)]
pub fn style_picker_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    current_style_name: String,
    mut is_style_picker_open: Signal<bool>,
    mut style_search_query: Signal<String>,
) -> Element {
    let all_entries = collect_style_names(&doc_state);
    let current = current_style_name;

    // Filter by search query (case-insensitive match against the display name).
    let query_lower = style_search_query.read().to_lowercase();
    let style_entries: Vec<(String, String)> = all_entries
        .into_iter()
        .filter(|(display, _)| {
            query_lower.is_empty() || display.to_lowercase().contains(&query_lower)
        })
        .collect();

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
                    aria_label: fl!("editor-style-picker-close-aria"),
                    onclick: move |_| {
                        style_search_query.set(String::new());
                        is_style_picker_open.set(false);
                    },
                    "✕"
                }
            }

            // ── Search input ──────────────────────────────────────────────────
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     padding: 0 {p}px {pb}px {p}px; flex-shrink: 0;",
                    p  = tokens::SPACE_4,
                    pb = tokens::SPACE_2,
                ),
                input {
                    r#type: "text",
                    value: "{style_search_query}",
                    placeholder: fl!("ribbon-style-search-placeholder"),
                    oninput: move |evt| style_search_query.set(evt.value()),
                    style: format!(
                        "flex: 1; height: 28px; padding: 0 {p}px; \
                         background: {bg}; border: 1px solid {border}; \
                         border-radius: {r}px; \
                         font-family: {ff}; font-size: {fs}px; \
                         color: {fg}; box-sizing: border-box;",
                        p      = tokens::SPACE_2,
                        bg     = tokens::COLOR_SURFACE_2,
                        border = tokens::COLOR_BORDER_DEFAULT,
                        r      = tokens::RADIUS_SM,
                        ff     = tokens::FONT_FAMILY_UI,
                        fs     = tokens::FONT_SIZE_BODY,
                        fg     = tokens::COLOR_TEXT_ON_CHROME,
                    ),
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

                {style_entries.into_iter().map(|(display_name, style_key)| {
                    let is_active = style_key == current;
                    let ds_click = Arc::clone(&doc_state);
                    let ds_preview = Arc::clone(&doc_state);
                    // style_key drives both the active check and the mutation dispatch.
                    let k = style_key.clone();
                    let dname = display_name.clone();
                    let (preview_fs, preview_fw, preview_fi) =
                        style_preview_font(&display_name, &style_key, &ds_preview);
                    rsx! {
                        button {
                            key: "{dname}",
                            style: format!(
                                "padding: {p}px {p2}px; border-radius: 4px; \
                                 border: 1px solid {border}; cursor: pointer; \
                                 font-family: {ff}; font-size: {fs}px; \
                                 font-weight: {fw}; font-style: {fi}; \
                                 background: {bg}; color: {fg}; flex-shrink: 0;",
                                p      = tokens::SPACE_1,
                                p2     = tokens::SPACE_2,
                                border = if is_active {
                                    tokens::COLOR_TAB_ACTIVE_INDICATOR
                                } else {
                                    tokens::COLOR_BORDER_CHROME
                                },
                                ff     = tokens::FONT_FAMILY_UI,
                                fs     = preview_fs,
                                fw     = preview_fw,
                                fi     = if preview_fi { "italic" } else { "normal" },
                                bg     = if is_active {
                                    tokens::COLOR_SURFACE_3
                                } else {
                                    tokens::COLOR_SURFACE_2
                                },
                                fg     = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            aria_label: fl!("ribbon-style-apply-aria", name = dname.clone()),
                            onclick: move |_| {
                                let ldoc_guard = loro_doc.read();
                                if let Some(ldoc) = ldoc_guard.as_ref()
                                    && let Some(focus) = cursor_state.read().focus.as_ref()
                                {
                                    let idx = focus.paragraph_index;
                                    if k == "Default Paragraph Style" {
                                        let _ = set_block_type_para(ldoc, idx);
                                    } else if let Some(lvl_str) = k.strip_prefix("Heading ") {
                                        if let Ok(level) = lvl_str.parse::<u8>() {
                                            let _ = set_block_type_heading(ldoc, idx, level);
                                        } else {
                                            let _ = set_block_style(ldoc, idx, &k);
                                        }
                                    } else {
                                        let _ = set_block_style(ldoc, idx, &k);
                                    }
                                    apply_mutation_and_relayout(&ds_click, ldoc);
                                }
                                post_mutation_sync(
                                    &ds_click,
                                    loro_doc,
                                    cursor_state,
                                    undo_manager,
                                    can_undo,
                                    can_redo,
                                );
                                style_search_query.set(String::new());
                                is_style_picker_open.set(false);
                            },
                            "{dname}"
                        }
                    }
                })}
            }
        }
    }
}
