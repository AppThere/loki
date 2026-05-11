// SPDX-License-Identifier: Apache-2.0

//! `AtDocumentTab` — a single closeable document tab for use inside [`AtTabBar`].

use dioxus::prelude::*;

use crate::tokens::colors::{
    COLOR_TAB_ACTIVE_BG, COLOR_TAB_ACTIVE_INDICATOR, COLOR_TAB_INACTIVE_HOVER,
    COLOR_TEXT_ON_CHROME, COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::layout::TAB_BAR_HEIGHT;
use crate::tokens::spacing::{RADIUS_SM, SPACE_1, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::{FONT_FAMILY_UI, FONT_SIZE_LABEL, FONT_WEIGHT_MEDIUM};

// ── AtDocumentTab ─────────────────────────────────────────────────────────────

/// A single closeable document tab inside [`crate::components::AtTabBar`].
///
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
///
/// The close button (20×20 px) is smaller than `TOUCH_MIN` in its visual
/// footprint; the enclosing tab (min 44 px height) satisfies the composite
/// touch target requirement for the overall interaction area.
///
/// Close button is 20×20 px within the tab; the tab itself (min 44 px height)
/// satisfies WCAG 2.5.8 for the composite interaction target.
#[component]
pub fn AtDocumentTab(props: AtDocumentTabProps) -> Element {
    let mut close_hovered = use_signal(|| false);
    let mut tab_hovered = use_signal(|| false);

    let tab_bg = if props.is_active {
        COLOR_TAB_ACTIVE_BG
    } else if tab_hovered() {
        COLOR_TAB_INACTIVE_HOVER
    } else {
        "transparent"
    };

    let close_bg = if close_hovered() {
        "#555555"
    } else {
        "transparent"
    };

    let mut label = if props.is_dirty {
        format!("• {}", props.title)
    } else {
        props.title.clone()
    };
    if props.is_discarded {
        // TODO(icons): replace 💤 Unicode fallback with a proper Tabler Icons
        // sleep/pause badge once an SVG icon system is implemented.
        label.push_str(" 💤");
    }

    let active_border = if props.is_active {
        format!("border-bottom: 2px solid {COLOR_TAB_ACTIVE_INDICATOR};")
    } else {
        String::new()
    };

    rsx! {
        div {
            role: "tab",
            "aria-selected": if props.is_active { "true" } else { "false" },
            style: format!(
                "display: flex; align-items: center; gap: {gap}px; \
                 height: {h}px; min-height: {touch}px; \
                 padding: 0 {px}px; flex-shrink: 0; \
                 background: {bg}; cursor: pointer; \
                 box-sizing: border-box; {active_border}",
                gap   = SPACE_1,
                h     = TAB_BAR_HEIGHT,
                touch = TOUCH_MIN,
                px    = SPACE_3,
                bg    = tab_bg,
            ),
            onmouseenter: move |_| { tab_hovered.set(true); },
            onmouseleave: move |_| { tab_hovered.set(false); },
            onclick: move |_| { props.on_activate.call(()); },

            // Tab title
            span {
                style: format!(
                    "font-size: {size}px; font-weight: {weight}; \
                     color: {fg}; max-width: 140px; overflow: hidden; \
                     font-family: {font};",
                    // COMPAT(dioxus-native): white-space: nowrap is unconfirmed —
                    // verify at runtime.
                    // COMPAT(dioxus-native): text-overflow: ellipsis is unconfirmed —
                    // verify at runtime.
                    size   = FONT_SIZE_LABEL,
                    weight = FONT_WEIGHT_MEDIUM,
                    fg     = if props.is_active {
                        COLOR_TEXT_ON_CHROME
                    } else {
                        COLOR_TEXT_ON_CHROME_SECONDARY
                    },
                    font   = FONT_FAMILY_UI,
                ),
                "{label}"
            }

            // Close button (×)
            button {
                "aria-label": "Close tab",
                style: format!(
                    "background: {bg}; border: none; border-radius: {r}px; \
                     color: {fg}; font-size: 12px; cursor: pointer; \
                     width: 20px; height: 20px; flex-shrink: 0; \
                     display: flex; align-items: center; justify-content: center; \
                     padding: 0;",
                    bg = close_bg,
                    r  = RADIUS_SM,
                    fg = COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                onmouseenter: move |_| { close_hovered.set(true); },
                onmouseleave: move |_| { close_hovered.set(false); },
                onclick: move |evt| {
                    evt.stop_propagation();
                    props.on_close.call(());
                },
                "×"
            }
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

/// Props for [`AtDocumentTab`].
#[derive(Props, Clone, PartialEq)]
pub struct AtDocumentTabProps {
    /// Displayed tab title.
    pub title: String,
    /// Whether this tab is the currently active one.
    pub is_active: bool,
    /// When `true`, prepend `"• "` to the title.
    pub is_dirty: bool,
    /// When `true`, append a sleep indicator to the title.
    pub is_discarded: bool,
    /// Callback when the tab is clicked.
    pub on_activate: EventHandler<()>,
    /// Callback when the close button is clicked.
    pub on_close: EventHandler<()>,
}
