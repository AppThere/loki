// SPDX-License-Identifier: Apache-2.0

//! Floating spelling-suggestions menu (the right-click context menu).
//!
//! Rendered as a `position: absolute` element with a full-size transparent
//! backdrop behind it, so a click anywhere outside dismisses the menu. Its
//! containing block is the editor root (`position: relative`) — which is *not*
//! the coordinate space the click arrives in; that is defect 3 below.
//!
//! # Three known defects, all live — **pending reroute**, not fixed
//!
//! Recorded so this file is not mistaken for a working reference — the r4
//! framing called it "the proven popup", and it is proven only in the region it
//! happened to be used in.
//!
//! **Status.** The replacement placement exists and is tested
//! (`editor_spell_place`) but has no caller: this file still positions itself,
//! so all three defects below are in the shipping app today. T4.1's `Popover`
//! closes them **when this file renders into `AtPopoverHost`** — mounted in
//! `app.rs` and waiting — and not before.
//!
//! 1. **No bottom-edge collision.** Placement is a horizontal clamp against
//!    `viewport_width` and `anchor_y.max(0.0)`; viewport *height* is not even a
//!    parameter. A menu opened near the bottom runs off it — measured at 298px
//!    of a 320px menu below the fold.
//! 2. **It is clipped by its own containing block.** The editor root carries
//!    `position: relative` **and `overflow: hidden`**
//!    (`editor_inner.rs`), and an out-of-flow child is clipped by an ancestor's
//!    overflow. So the overflow in (1) is not merely off-screen, it is *cut* —
//!    which is why the symptom reads as a truncated menu rather than one that
//!    obviously ran off, and why no `z-index` would have helped.
//!
//! 3. **It is displaced downward by the shell chrome.** The anchor is
//!    `client_x/client_y`, which in this stack is **window**-relative (plus a
//!    top-level scroll that is always zero here) — see "Documented stack
//!    deviations" in `docs/patches.md`. But it is used as `top`/`left` on an
//!    element whose containing block is the **editor root**, and the editor root
//!    is not at the window origin: `app.rs` root → `Shell` → `AtTabBar`
//!    (`TAB_BAR_HEIGHT` 40px + 1px bottom border) → Outlet container →
//!    `EditorInner`'s `position: relative` div. So the menu renders about
//!    **41px below the click** on desktop, and further on Android where the top
//!    safe-area inset adds.
//!
//!    A downward offset of that size is why nobody has reported it: a context
//!    menu appearing just under the cursor looks like ordinary behaviour. It is
//!    the one of the three that would have survived a screen session.
//!
//! `appthere_ui::components::popover` closes all three: it flips at the bottom
//! edge, hosts outside every clipping ancestor, and works in window coordinates
//! throughout. `TODO(t4.1-popover): migrate this to the shared primitive.`

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
