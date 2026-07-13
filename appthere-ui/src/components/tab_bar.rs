// SPDX-License-Identifier: Apache-2.0

//! `AtTabBar` — document tab bar shell component.
//!
//! Renders a full-width horizontal strip containing:
//! - A fixed **Home tab** (always present, cannot be closed).
//! - Zero or more **document tabs** ([`AtDocumentTab`]), horizontally scrollable.
//! - A **New tab** (`+`) button at the far right.
//!
//! Individual tab rendering is split into [`crate::components::document_tab`].

use dioxus::prelude::*;

use crate::components::document_tab::AtDocumentTab;
use crate::theme::use_theme;
use crate::tokens::layout::TAB_BAR_HEIGHT;
use crate::tokens::spacing::{SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::{FONT_FAMILY_UI, FONT_SIZE_LABEL, FONT_WEIGHT_SEMIBOLD};

// ── AtDocumentTabData ─────────────────────────────────────────────────────────

/// Data describing a single open document tab (non-component data struct).
#[derive(Clone, PartialEq, Debug)]
pub struct AtDocumentTabData {
    /// Displayed tab title (document filename or unsaved title).
    pub title: String,
    /// When `true`, prepend `"• "` to signal unsaved changes.
    pub is_dirty: bool,
    /// When `true`, the tab has been evicted from memory.
    pub is_discarded: bool,
}

// ── AtTabBar ──────────────────────────────────────────────────────────────────

/// Document tab bar shell component.
///
/// Always renders a fixed Home tab at index 0, followed by document tabs,
/// followed by a New Tab button.
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
#[component]
pub fn AtTabBar(props: AtTabBarProps) -> Element {
    let mut theme = use_theme();
    let palette = theme.palette();
    let mut home_hovered = use_signal(|| false);
    let mut new_tab_hovered = use_signal(|| false);
    let mut theme_hovered = use_signal(|| false);

    let home_is_active = props.active_index == 0;
    let home_bg = if home_is_active {
        palette.tab_active_bg
    } else if home_hovered() {
        palette.tab_inactive_hover
    } else {
        "transparent"
    };
    let home_border = if home_is_active {
        format!("border-bottom: 2px solid {};", palette.tab_active_indicator)
    } else {
        String::new()
    };

    let new_tab_bg = if new_tab_hovered() {
        palette.tab_inactive_hover
    } else {
        "transparent"
    };
    let theme_bg = if theme_hovered() {
        palette.tab_inactive_hover
    } else {
        "transparent"
    };

    rsx! {
        div {
            role: "tablist",
            "aria-label": props.aria_label,
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 background: {bg}; border-bottom: 1px solid {border}; \
                 display: flex; flex-direction: row; align-items: center; \
                 flex-shrink: 0; font-family: {font}; \
                 overflow-x: auto; overflow-y: hidden;",
                // COMPAT(dioxus-native): overflow-x: auto is confirmed working.
                // overflow-y: hidden clips any child that exceeds TAB_BAR_HEIGHT
                // (e.g. min-height: TOUCH_MIN on tab items).
                // scrollbar-width: none is unconfirmed — verify at runtime.
                h      = TAB_BAR_HEIGHT,
                bg     = palette.surface_chrome,
                border = palette.border_chrome,
                font   = FONT_FAMILY_UI,
            ),

            // ── Home tab (always present, cannot be closed) ───────────────────
            div {
                role: "tab",
                "aria-selected": if home_is_active { "true" } else { "false" },
                style: format!(
                    "display: flex; align-items: center; \
                     height: {h}px; min-height: {touch}px; \
                     padding: 0 {px}px; flex-shrink: 0; min-width: 72px; \
                     background: {bg}; cursor: pointer; \
                     box-sizing: border-box; {active_border}",
                    h             = TAB_BAR_HEIGHT,
                    touch         = TOUCH_MIN,
                    px            = SPACE_3,
                    bg            = home_bg,
                    active_border = home_border,
                ),
                onmouseenter: move |_| { home_hovered.set(true); },
                onmouseleave: move |_| { home_hovered.set(false); },
                onclick: move |_| { props.on_tab_select.call(0); },

                span {
                    style: format!(
                        "font-size: {size}px; font-weight: {weight}; color: {fg}; \
                         white-space: nowrap; overflow: hidden;",
                        // COMPAT(dioxus-native): white-space: nowrap is unconfirmed
                        // — verify at runtime.
                        size   = FONT_SIZE_LABEL,
                        weight = FONT_WEIGHT_SEMIBOLD,
                        fg     = if home_is_active {
                            palette.text_on_chrome
                        } else {
                            palette.text_on_chrome_secondary
                        },
                    ),
                    "{props.home_tab_label}"
                }
            }

            // ── Document tabs (horizontally scrollable) ───────────────────────
            for (idx, tab) in props.tabs.iter().enumerate() {
                {
                    let doc_idx = idx + 1;
                    let title = tab.title.clone();
                    let is_dirty = tab.is_dirty;
                    let is_discarded = tab.is_discarded;
                    let is_active = props.active_index == doc_idx;
                    rsx! {
                        AtDocumentTab {
                            key: "{doc_idx}",
                            title: title,
                            is_active: is_active,
                            is_dirty: is_dirty,
                            is_discarded: is_discarded,
                            on_activate: move |_| { props.on_tab_select.call(doc_idx); },
                            on_close: move |_| { props.on_tab_close.call(doc_idx); },
                        }
                    }
                }
            }

            // Flex spacer — pushes the new-tab button to the right
            div { style: "flex: 1;" }

            // ── New tab (+) button ────────────────────────────────────────────
            // Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).
            button {
                "aria-label": props.new_tab_aria_label,
                style: format!(
                    "background: {bg}; border: none; \
                     color: {fg}; font-size: 18px; cursor: pointer; \
                     width: 32px; height: {touch}px; flex-shrink: 0; \
                     display: flex; align-items: center; justify-content: center; \
                     padding: 0 {p}px;",
                    bg    = new_tab_bg,
                    fg    = palette.text_accent,
                    touch = TOUCH_MIN,
                    p     = SPACE_2,
                ),
                onmouseenter: move |_| { new_tab_hovered.set(true); },
                onmouseleave: move |_| { new_tab_hovered.set(false); },
                onclick: move |_| { props.on_new_tab.call(()); },
                "+"
            }

            // ── Theme toggle (Dark ⇄ Light) ───────────────────────────────────
            // Rendered only when the app supplies an aria label (i18n rule:
            // display strings are props). Self-contained: flips the shared
            // AtThemeContext signal, so every palette-reading component
            // re-colors live. 44 px touch target (WCAG 2.5.8).
            if !props.theme_toggle_aria_label.is_empty() {
                button {
                    "aria-label": props.theme_toggle_aria_label,
                    style: format!(
                        "background: {bg}; border: none; \
                         color: {fg}; font-size: 14px; cursor: pointer; \
                         width: 32px; height: {touch}px; flex-shrink: 0; \
                         display: flex; align-items: center; justify-content: center; \
                         padding: 0 {p}px;",
                        bg    = theme_bg,
                        fg    = palette.text_on_chrome_secondary,
                        touch = TOUCH_MIN,
                        p     = SPACE_2,
                    ),
                    onmouseenter: move |_| { theme_hovered.set(true); },
                    onmouseleave: move |_| { theme_hovered.set(false); },
                    onclick: move |_| { theme.toggle(); },
                    "◐"
                }
            }
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

/// Props for [`AtTabBar`].
#[derive(Props, Clone, PartialEq)]
pub struct AtTabBarProps {
    /// Document tabs (excluding the Home tab, which is always prepended).
    pub tabs: Vec<AtDocumentTabData>,
    /// Active tab index. `0` = Home tab; `1..=N` = document tabs.
    pub active_index: usize,
    /// Label for the always-present Home tab.
    pub home_tab_label: String,
    /// Aria label for the tab list container.
    pub aria_label: String,
    /// Callback when a tab is selected. Argument is the tab index (0 = Home).
    pub on_tab_select: EventHandler<usize>,
    /// Callback when a document tab's close button is clicked.
    /// Argument is the tab index (always ≥ 1; the Home tab cannot be closed).
    pub on_tab_close: EventHandler<usize>,
    /// Callback when the new-tab (`+`) button is clicked.
    pub on_new_tab: EventHandler<()>,
    /// Aria label for the new-tab button.
    pub new_tab_aria_label: String,
    /// Aria label for the Dark ⇄ Light theme toggle. Empty (the default)
    /// hides the toggle, so existing call sites are unchanged.
    #[props(default)]
    pub theme_toggle_aria_label: String,
}
