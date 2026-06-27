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
/// Pass an [`crate::components::icons::AtIcon`] as the `children` slot to
/// render a Lucide SVG icon.  When `children` is empty, the button renders
/// as a blank 44×44 press target (aria_label is still announced).
///
/// # COMPAT(dioxus-native): SVG rendering via Blitz is unconfirmed.
/// If SVG icons do not render, `aria_label` provides the accessible name and
/// the button remains functional.
///
/// # Hover state
///
/// // COMPAT(dioxus-native): CSS `:hover` is unsupported in Blitz. Hover
/// // background is implemented via `onmouseenter`/`onmouseleave` signals,
/// // matching the pattern in `tab_strip.rs`.
#[component]
pub fn AtRibbonIconButton(
    /// Full accessible name for screen readers and tooltips.
    aria_label: String,
    /// Whether this button is in the active/toggled state
    /// (e.g. Bold is active when the cursor is in bold text).
    is_active: bool,
    /// Whether this button is disabled (not interactive).
    is_disabled: bool,
    /// Callback when the button is clicked or tapped.
    on_click: EventHandler<()>,
    /// Icon content to render inside the button (typically an [`AtIcon`]).
    children: Element,
) -> Element {
    let mut hovered = use_signal(|| false);

    let bg = if is_active {
        tokens::COLOR_TAB_ACTIVE_BG
    } else if hovered() {
        tokens::COLOR_TAB_INACTIVE_HOVER
    } else {
        "transparent"
    };

    let icon_color = if is_disabled {
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
                 color: {fg};",
                touch  = tokens::TOUCH_MIN,
                radius = tokens::RADIUS_MD,
                bg     = bg,
                fg     = icon_color,
            ),
            aria_label:   aria_label.clone(),
            // The hover tooltip overlay (blitz-shell) reads `title`; reuse the
            // accessible name so every icon button is self-describing on hover.
            title:        aria_label.clone(),
            aria_pressed: if is_active { "true" } else { "false" },
            disabled:     is_disabled,
            onmouseenter: move |_| hovered.set(true),
            onmouseleave: move |_| hovered.set(false),
            onclick: move |_| {
                if !is_disabled {
                    on_click.call(());
                }
            },
            {children}
        }
    }
}
