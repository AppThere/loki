// SPDX-License-Identifier: Apache-2.0

//! Floating spelling-suggestions menu (the right-click context menu).
//!
//! Rendered as a `position: absolute` element anchored at the cursor — verified
//! to work in the current Blitz stack (Stylo + stylo_taffy + Taffy 0.9). A
//! full-size transparent backdrop sits behind it so a click anywhere outside
//! dismisses the menu. The editor root is `position: relative`, so the menu's
//! containing block is the editor area and the click's window-relative
//! coordinates place it at the cursor.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;
use loki_i18n::fl;

use crate::editing::state::DocumentState;
use crate::routes::editor::editor_spell::{
    SpellMenu, SpellSync, add_to_dictionary, ignore_word, replace_word,
};

/// Width of the floating menu in CSS pixels.
const MENU_WIDTH_PX: f32 = 300.0;
/// Maximum height before the menu scrolls.
const MENU_MAX_HEIGHT_PX: f32 = 320.0;
/// Gap kept between the menu and the viewport's right edge when clamping.
const EDGE_MARGIN_PX: f32 = 8.0;

/// Renders the floating suggestions menu when `spell_menu` is `Some`.
///
/// `spell_hover` holds the key of the menu row currently under the pointer.
/// Blitz dispatches no `mouseenter`/`mouseleave` (and no CSS `:hover`), so hover
/// is tracked from `onmousemove` on each row — entering a row sets its key,
/// moving over the backdrop clears it — and applied as an inline background.
pub(super) fn spelling_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    sync: SpellSync,
    service: SpellService,
    mut spell_menu: Signal<Option<SpellMenu>>,
    mut is_language_panel_open: Signal<bool>,
    viewport_width: f32,
    spell_hover: Signal<Option<String>>,
) -> Element {
    let Some(menu) = spell_menu.read().clone() else {
        return rsx! {};
    };

    // Clamp horizontally so the menu never spills off the measured viewport's
    // right edge.
    let max_left = (viewport_width - MENU_WIDTH_PX - EDGE_MARGIN_PX).max(0.0);
    let left = menu.anchor_x.clamp(0.0, max_left);
    let top = menu.anchor_y.max(0.0);

    rsx! {
        // Backdrop: a transparent full-area layer that dismisses on click.
        // Moving over it (i.e. off any row) clears the hover highlight, since
        // Blitz delivers no `mouseleave`.
        div {
            style: "position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 1000;",
            onclick: move |_| { spell_menu.set(None); },
            onmousemove: {
                let mut spell_hover = spell_hover;
                move |_| {
                    if spell_hover.peek().is_some() {
                        spell_hover.set(None);
                    }
                }
            },
        }

        // The menu itself, anchored at the cursor.
        div {
            style: format!(
                "position: absolute; left: {left}px; top: {top}px; z-index: 1001; \
                 width: {w}px; max-height: {mh}px; box-sizing: border-box; \
                 display: flex; flex-direction: column; \
                 background: {bg}; border: 1px solid {border}; border-radius: 6px; \
                 overflow-y: auto; overflow-x: hidden; padding: {pad}px;",
                w = MENU_WIDTH_PX,
                mh = MENU_MAX_HEIGHT_PX,
                bg = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
                pad = tokens::SPACE_2,
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
                        ff = tokens::FONT_FAMILY_UI,
                        size = tokens::FONT_SIZE_LABEL,
                        fg = tokens::COLOR_TEXT_ON_CHROME,
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

            // Suggestions — a vertical list; click to replace the word.
            if menu.misspelled && !menu.suggestions.is_empty() {
                for (i, suggestion) in menu.suggestions.clone().into_iter().enumerate() {
                    {
                        let doc_state = Arc::clone(&doc_state);
                        let menu = menu.clone();
                        let label = suggestion.clone();
                        let key = format!("sug-{i}");
                        let hovered = is_hovered(spell_hover, &key);
                        rsx! {
                            button {
                                style: menu_item_style(hovered),
                                onmousemove: hover_setter(spell_hover, key),
                                onclick: move |_| {
                                    replace_word(&doc_state, sync, &menu, &suggestion);
                                    spell_menu.set(None);
                                },
                                "{label}"
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

            // Separator before the actions.
            div {
                style: format!(
                    "border-top: 1px solid {border}; margin: {m}px 0;",
                    border = tokens::COLOR_BORDER_CHROME,
                    m = tokens::SPACE_1,
                ),
            }

            // Actions.
            if menu.misspelled {
                {
                    let doc_state = Arc::clone(&doc_state);
                    let service = service.clone();
                    let word = menu.word.clone();
                    let cursor_state = sync.cursor_state;
                    rsx! {
                        button {
                            style: menu_item_style(is_hovered(spell_hover, "add")),
                            onmousemove: hover_setter(spell_hover, "add".to_string()),
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
                            style: menu_item_style(is_hovered(spell_hover, "ignore")),
                            onmousemove: hover_setter(spell_hover, "ignore".to_string()),
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
                style: menu_item_style(is_hovered(spell_hover, "lang")),
                onmousemove: hover_setter(spell_hover, "lang".to_string()),
                onclick: move |_| {
                    spell_menu.set(None);
                    is_language_panel_open.set(true);
                },
                {fl!("editor-spelling-language")}
            }
        }
    }
}

/// A full-width, left-aligned menu row. `hovered` tints the background (Blitz
/// has no CSS `:hover`).
fn menu_item_style(hovered: bool) -> String {
    let bg = if hovered {
        tokens::COLOR_SURFACE_3
    } else {
        "transparent"
    };
    format!(
        "display: block; width: 100%; text-align: left; \
         padding: {p}px {p2}px; background: {bg}; border: none; \
         border-radius: 4px; color: {fg}; font-family: {ff}; \
         font-size: {size}px; cursor: pointer;",
        p = tokens::SPACE_1,
        p2 = tokens::SPACE_2,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        ff = tokens::FONT_FAMILY_UI,
        size = tokens::FONT_SIZE_LABEL,
    )
}

/// Whether the row with `key` is currently hovered.
fn is_hovered(spell_hover: Signal<Option<String>>, key: &str) -> bool {
    spell_hover.read().as_deref() == Some(key)
}

/// Builds an `onmousemove` handler that marks `key` as the hovered row. Guards
/// with `peek()` so a move *within* the same row does not trigger a redundant
/// signal write (and re-render); only crossing into a new row updates state.
fn hover_setter(
    mut spell_hover: Signal<Option<String>>,
    key: String,
) -> impl FnMut(Event<MouseData>) {
    move |_| {
        if spell_hover.peek().as_deref() != Some(key.as_str()) {
            spell_hover.set(Some(key.clone()));
        }
    }
}

fn muted_text_style() -> String {
    format!(
        "font-family: {ff}; font-size: {size}px; color: {fg}; padding: {p}px {p2}px;",
        ff = tokens::FONT_FAMILY_UI,
        size = tokens::FONT_SIZE_LABEL,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
        p = tokens::SPACE_1,
        p2 = tokens::SPACE_2,
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
