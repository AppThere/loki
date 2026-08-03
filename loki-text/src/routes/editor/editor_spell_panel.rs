// SPDX-License-Identifier: Apache-2.0

//! Floating spelling-suggestions menu (the right-click context menu).
//!
//! **This file renders the menu's *contents*. It does not place them and it owns
//! no backdrop** — both are `AtPopoverHost`'s at the app root, since r63/r64.
//! `editor_spell_popover` hands the host a request built by `editor_spell_place`;
//! what is left here is the rows.
//!
//! # The three defects this file used to have are closed (r63) — kept as history
//!
//! The header below described them as **live in the shipping app** until r73,
//! ten commits after the migration that closed them. That is worth recording
//! rather than deleting: the correction went into `editor_spell_place`'s status
//! section and into a comment sixty lines further down *this* file, while the
//! module header — the thing a reader opens when they look up the spelling menu —
//! kept the pre-migration account. Right content, wrong place, which is L08-053
//! and the reason this audit exists.
//!
//! What they were, and what closed each:
//!
//! 1. **No bottom-edge collision.** Placement was a horizontal clamp with
//!    viewport *height* not even a parameter; a menu opened near the bottom ran
//!    298px of its 320px below the fold. `popover::place` flips.
//! 2. **Clipped by its own containing block.** The editor root carries
//!    `position: relative` **and `overflow: hidden`**, and an out-of-flow child
//!    is clipped by an ancestor's overflow — so the overflow in (1) was *cut*,
//!    not merely off-screen, and no `z-index` would have helped. The host is a
//!    child of the app root, outside every clipping ancestor.
//! 3. **Displaced ~41px downward by the shell chrome.** The anchor is
//!    `client_x/client_y`, window-relative in this stack (see "Documented stack
//!    deviations" in `docs/patches.md`), but it was used as `top`/`left` against
//!    the **editor root**, which sits below `AtTabBar` (40px + 1px border). The
//!    host's containing block is the app root, whose padding box starts at the
//!    window origin, so the two spaces now agree.
//!
//!    That third one is the one that would have survived a screen session — a
//!    context menu appearing just under the cursor looks like ordinary
//!    behaviour. It is why `spell_menu_anchor` is a tested function rather than
//!    two field reads.
//!
//! **Not established by any of this:** that the menu lands on the word on a real
//! screen. The arithmetic and the absence of a scroll term in our path are
//! tested; the platform half is the scroll-drift sitting, whose procedure and
//! three readings are in `editor_spell_place`.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;
use loki_i18n::fl;

use crate::editing::state::DocumentState;
use crate::routes::editor::editor_spell::{
    SpellMenu, SpellSync, add_to_dictionary, ignore_word, replace_word,
};

// The width, max height and edge margin now live only in `editor_spell_place`,
// which is where they belong: the host sizes the container from the resolved
// placement, so a constant used here as well would be a second opinion about the
// same measurement.

/// Renders the floating suggestions menu when `spell_menu` is `Some`.
///
/// `spell_hover` holds the key of the menu row currently under the pointer.
/// Blitz dispatches no `mouseenter`/`mouseleave` (and no CSS `:hover`), so hover
/// is tracked from `onmousemove` on each row — entering a row sets its key,
/// moving over the backdrop clears it — and applied as an inline background.
/// The menu's **content**, with no position of its own (Spec 08 T4.1, r63).
///
/// # What left this function, and why that is the migration
///
/// It used to place itself — a horizontal clamp against the editor's measured
/// width, `anchor_y.max(0.0)` vertically, and a backdrop at `z-index: 1000`. All
/// three of the defects recorded above lived in those four lines. Placement is
/// now `editor_spell_place` → `popover::place`, resolved once by
/// `AtPopoverContext::open_resolved` and rendered by `AtPopoverHost` at the app
/// root, which is outside every clipping ancestor and in the same coordinate
/// space as the click.
///
/// So this builds rows and nothing else. It is **invoked by the host during the
/// host's render** (see `PopoverRequest::content`), which is what keeps the row
/// highlight live: reading `spell_hover` here subscribes the host, so a pointer
/// move re-renders the menu.
pub(super) fn spell_menu_content(
    doc_state: Arc<Mutex<DocumentState>>,
    sync: SpellSync,
    service: SpellService,
    mut spell_menu: Signal<Option<SpellMenu>>,
    mut is_language_panel_open: Signal<bool>,
    spell_hover: Signal<Option<String>>,
) -> Element {
    let Some(menu) = spell_menu.read().clone() else {
        return rsx! {};
    };

    rsx! {
        div {
            style: format!(
                "width: 100%; box-sizing: border-box; \
                 display: flex; flex-direction: column; \
                 background: {bg}; border: 1px solid {border}; border-radius: 6px; \
                 overflow-x: hidden; padding: {pad}px;",
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
                        "font-size: {size}px; color: {fg}; font-weight: 600;",
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
         border-radius: 4px; color: {fg}; \
         font-size: {size}px; cursor: pointer;",
        p = tokens::SPACE_1,
        p2 = tokens::SPACE_2,
        fg = tokens::COLOR_TEXT_ON_CHROME,
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
        "font-size: {size}px; color: {fg}; padding: {p}px {p2}px;",
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
