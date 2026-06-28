// SPDX-License-Identifier: Apache-2.0

//! Inline spelling-suggestions panel ("context menu") for the document editor.
//!
//! `position: absolute` is unsupported in Blitz (see editor_style.rs), so the
//! right-click suggestions UI is rendered as a docked panel between the canvas
//! and the ribbon — the same pattern as the style picker. It shows the word
//! under the cursor, ranked suggestions (click to replace), and the
//! Add-to-Dictionary / Ignore / change-language actions.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;
use loki_i18n::fl;

use crate::editing::state::DocumentState;
use crate::routes::editor::editor_spell::{
    SpellMenu, SpellSync, add_to_dictionary, ignore_word, replace_word,
};

/// Height of the open spelling panel in CSS pixels.
pub(super) const SPELL_PANEL_HEIGHT_PX: f32 = 150.0;

/// Renders the spelling panel when `spell_menu` is `Some`.
pub(super) fn spelling_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    sync: SpellSync,
    service: SpellService,
    mut spell_menu: Signal<Option<SpellMenu>>,
    mut is_language_panel_open: Signal<bool>,
) -> Element {
    let Some(menu) = spell_menu.read().clone() else {
        return rsx! {};
    };

    rsx! {
        div {
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: column; flex-shrink: 0; \
                 background: {bg}; border-top: 1px solid {border}; \
                 overflow-y: auto; overflow-x: hidden; padding: {pad}px;",
                h      = SPELL_PANEL_HEIGHT_PX,
                bg     = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
                pad    = tokens::SPACE_2,
            ),

            // Header: the word + close button.
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     justify-content: space-between; margin-bottom: {mb}px;",
                    mb = tokens::SPACE_2,
                ),
                span {
                    style: format!(
                        "font-family: {ff}; font-size: {size}px; color: {fg}; font-weight: 600;",
                        ff   = tokens::FONT_FAMILY_UI,
                        size = tokens::FONT_SIZE_LABEL,
                        fg   = tokens::COLOR_TEXT_ON_CHROME,
                    ),
                    if menu.misspelled {
                        {fl!("editor-spelling-heading", word = menu.word.clone())}
                    } else {
                        {fl!("editor-spelling-correct", word = menu.word.clone())}
                    }
                }
                button {
                    style: close_button_style(),
                    onclick: move |_| { spell_menu.set(None); },
                    "\u{2715}"
                }
            }

            // Suggestions (clickable) — replace the word on click.
            if menu.misspelled && !menu.suggestions.is_empty() {
                div {
                    style: "display: flex; flex-direction: row; flex-wrap: wrap; gap: 6px;",
                    for suggestion in menu.suggestions.clone() {
                        {
                            let doc_state = Arc::clone(&doc_state);
                            let menu = menu.clone();
                            let label = suggestion.clone();
                            rsx! {
                                button {
                                    style: chip_style(),
                                    onclick: move |_| {
                                        replace_word(&doc_state, sync, &menu, &suggestion);
                                        spell_menu.set(None);
                                    },
                                    "{label}"
                                }
                            }
                        }
                    }
                }
            } else if menu.misspelled {
                span {
                    style: muted_text_style(),
                    {fl!("editor-spelling-no-suggestions")}
                }
            }

            // Action row.
            div {
                style: format!(
                    "display: flex; flex-direction: row; flex-wrap: wrap; gap: 6px; \
                     margin-top: auto; padding-top: {pt}px;",
                    pt = tokens::SPACE_2,
                ),
                if menu.misspelled {
                    {
                        let doc_state = Arc::clone(&doc_state);
                        let service = service.clone();
                        let word = menu.word.clone();
                        let cursor_state = sync.cursor_state;
                        rsx! {
                            button {
                                style: action_style(),
                                onclick: move |_| {
                                    add_to_dictionary(&doc_state, cursor_state, &service, &word);
                                    spell_menu.set(None);
                                },
                                {fl!("editor-spelling-add-dictionary")}
                            }
                        }
                    }
                    {
                        let doc_state = Arc::clone(&doc_state);
                        let service = service.clone();
                        let word = menu.word.clone();
                        let cursor_state = sync.cursor_state;
                        rsx! {
                            button {
                                style: action_style(),
                                onclick: move |_| {
                                    ignore_word(&doc_state, cursor_state, &service, &word);
                                    spell_menu.set(None);
                                },
                                {fl!("editor-spelling-ignore")}
                            }
                        }
                    }
                }
                button {
                    style: action_style(),
                    onclick: move |_| {
                        spell_menu.set(None);
                        is_language_panel_open.set(true);
                    },
                    {fl!("editor-spelling-language")}
                }
            }
        }
    }
}

fn chip_style() -> String {
    format!(
        "padding: {p}px {p2}px; background: {bg}; border: 1px solid {border}; \
         border-radius: 4px; color: {fg}; font-size: {size}px; cursor: pointer;",
        p = tokens::SPACE_1,
        p2 = tokens::SPACE_3,
        bg = tokens::COLOR_SURFACE_3,
        border = tokens::COLOR_BORDER_CHROME,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    )
}

fn action_style() -> String {
    format!(
        "padding: {p}px {p2}px; background: {bg}; border: 1px solid {border}; \
         border-radius: 4px; color: {fg}; font-size: {size}px; cursor: pointer;",
        p = tokens::SPACE_1,
        p2 = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
        size = tokens::FONT_SIZE_LABEL,
    )
}

fn muted_text_style() -> String {
    format!(
        "font-family: {ff}; font-size: {size}px; color: {fg};",
        ff = tokens::FONT_FAMILY_UI,
        size = tokens::FONT_SIZE_LABEL,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
    )
}

fn close_button_style() -> String {
    format!(
        "background: transparent; border: none; font-size: {fs}px; color: {fg}; \
         cursor: pointer; padding: {p}px;",
        fs = tokens::FONT_SIZE_LABEL,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
        p = tokens::SPACE_1,
    )
}
