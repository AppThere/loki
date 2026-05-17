// SPDX-License-Identifier: Apache-2.0

//! Ribbon button components.
//!
//! # Touch target
//!
//! All interactive elements are sized to [`tokens::TOUCH_MIN`] × [`tokens::TOUCH_MIN`]
//! logical pixels (44×44 px) to satisfy WCAG 2.5.8.

use dioxus::prelude::*;

use crate::tokens;

/// A compact icon-only ribbon button used for well-known toggle actions
/// (Bold, Italic, Underline, etc.).
///
/// # Touch target
///
/// Width and height are both [`tokens::TOUCH_MIN`] (44×44 logical pixels),
/// satisfying WCAG 2.5.8.
///
/// # Icons
///
/// `icon_label` is currently a text label (e.g. "B", "I", "U").
/// // TODO(icons): Replace text labels with Tabler Icons SVGs once an icon
/// // system pass is completed.
///
/// # Hover state
///
/// // COMPAT(dioxus-native): CSS `:hover` is unsupported in Blitz. Hover
/// // background is implemented via `onmouseenter`/`onmouseleave` signals,
/// // matching the pattern in `tab_strip.rs`.
#[component]
pub fn AtRibbonIconButton(
    /// Short visible label (e.g. "B" for Bold, "I" for Italic).
    /// Will be replaced by an SVG icon in a future pass.
    icon_label: String,
    /// Full accessible name for screen readers and tooltips.
    aria_label: String,
    /// Whether this button is in the active/toggled state
    /// (e.g. Bold is active when the cursor is in bold text).
    is_active: bool,
    /// Whether this button is disabled (not interactive).
    is_disabled: bool,
    /// Callback when the button is clicked or tapped.
    on_click: EventHandler<()>,
) -> Element {
    let mut hovered = use_signal(|| false);

    let bg = if is_active {
        tokens::COLOR_TAB_ACTIVE_BG
    } else if hovered() {
        tokens::COLOR_TAB_INACTIVE_HOVER
    } else {
        "transparent"
    };

    let text_color = if is_disabled {
        tokens::COLOR_ICON_DISABLED
    } else if is_active {
        tokens::COLOR_TEXT_ACCENT
    } else {
        tokens::COLOR_TEXT_ON_CHROME
    };

    rsx! {
        button {
            style: format!(
                "width: {touch}px; height: {touch}px; \
                 display: flex; align-items: center; justify-content: center; \
                 background: {bg}; border: none; \
                 border-radius: {radius}px; cursor: pointer; \
                 color: {fg}; \
                 font-family: {font}; font-size: {size}px; font-weight: {weight};",
                touch  = tokens::TOUCH_MIN,
                radius = tokens::RADIUS_MD,
                bg     = bg,
                fg     = text_color,
                font   = tokens::FONT_FAMILY_UI,
                size   = tokens::FONT_SIZE_BODY,
                weight = tokens::FONT_WEIGHT_BOLD,
            ),
            aria_label:   aria_label.clone(),
            aria_pressed: if is_active { "true" } else { "false" },
            disabled:     is_disabled,
            onmouseenter: move |_| hovered.set(true),
            onmouseleave: move |_| hovered.set(false),
            onclick: move |_| {
                if !is_disabled {
                    on_click.call(());
                }
            },
            "{icon_label}"
        }
    }
}
