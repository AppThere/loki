// SPDX-License-Identifier: Apache-2.0

//! `AtTitleBar` — application title bar shell component.
//!
//! Renders on all platforms. The [`Platform`] prop controls height and
//! left-padding to avoid overlapping OS window decorations.

use dioxus::prelude::*;

use crate::components::platform::Platform;
use crate::responsive::use_breakpoint;
use crate::tokens::colors::{
    COLOR_BORDER_CHROME, COLOR_SURFACE_CHROME, COLOR_TEXT_ACCENT, COLOR_TEXT_ON_CHROME,
    COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::layout::{TITLE_BAR_HEIGHT_DEFAULT, TITLE_BAR_HEIGHT_MACOS};
use crate::tokens::spacing::{
    ICON_SIZE_LG, RADIUS_SM, SPACE_1, SPACE_10, SPACE_2, SPACE_3, TOUCH_MIN,
};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_BOLD, FONT_WEIGHT_SEMIBOLD,
};

// ── AtTitleBar ────────────────────────────────────────────────────────────────

/// Application title bar shell component.
///
/// Renders across the full width of the window at the top of the application.
/// Adapts its height and left-padding based on the host [`Platform`].
///
/// # Layout
///
/// `[icon | title (center, flex:1) | collaborator badge]`
///
/// The center region acts as a drag target placeholder.
///
/// # Touch target
///
/// The icon button (left slot) meets the minimum interactive size:
/// **Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).**
#[component]
pub fn AtTitleBar(props: AtTitleBarProps) -> Element {
    let bar_height = if props.platform == Platform::MacOs {
        TITLE_BAR_HEIGHT_MACOS
    } else {
        TITLE_BAR_HEIGHT_DEFAULT
    };

    // macOS: leave SPACE_10 (40px) on the left so traffic light buttons are
    // not obscured by the icon.
    // TODO(platform): macOS traffic light region — the window decoration
    // buttons are managed by blitz-shell at the window layer. This component
    // leaves left padding of SPACE_10 on MacOs to avoid overlapping them.
    let left_pad = if props.platform == Platform::MacOs {
        SPACE_10
    } else {
        SPACE_2
    };

    let mut icon_hovered = use_signal(|| false);
    let icon_bg = if icon_hovered() {
        "#2E2E2E"
    } else {
        "transparent"
    };

    // Compact (phone-width): drop redundant chrome so the title isn't crowded.
    let compact = use_breakpoint().is_compact();

    let title_text = match &props.document_title {
        Some(t) if props.is_dirty => format!("• {t}"),
        Some(t) => t.clone(),
        None => props.app_name.to_string(),
    };

    rsx! {
        div {
            // role="banner" — marks this region as the page header landmark.
            // COMPAT(dioxus-native): Dioxus 0.7 RSX uses `role` as the attribute name.
            // If the Blitz HTML parser does not honor it, the layout is unaffected.
            role: "banner",
            style: format!(
                "height: {h}px; min-height: {h}px; background: {bg}; \
                 border-bottom: 1px solid {border}; \
                 display: flex; align-items: center; \
                 padding: 0 {pr}px 0 {pl}px; flex-shrink: 0; gap: {gap}px; \
                 font-family: {font};",
                h      = bar_height,
                bg     = COLOR_SURFACE_CHROME,
                border = COLOR_BORDER_CHROME,
                pr     = SPACE_3,
                pl     = left_pad,
                gap    = SPACE_2,
                font   = FONT_FAMILY_UI,
            ),

            // ── App icon button ───────────────────────────────────────────────
            // Minimum interactive size: 44×44 logical pixels (WCAG 2.5.8).
            button {
                "aria-label": props.app_name,
                style: format!(
                    "background: {bg}; border: none; border-radius: {r}px; \
                     cursor: pointer; padding: 0; flex-shrink: 0; \
                     width: {sz}px; height: {touch}px; \
                     display: flex; align-items: center; justify-content: center;",
                    bg    = icon_bg,
                    r     = RADIUS_SM,
                    sz    = ICON_SIZE_LG,
                    touch = TOUCH_MIN,
                ),
                onmouseenter: move |_| { icon_hovered.set(true); },
                onmouseleave: move |_| { icon_hovered.set(false); },
                onclick: move |_| { props.on_icon_press.call(()); },

                // App icon placeholder — colored square.
                // TODO(icons): Replace with an SVG app icon asset once the icon
                // system is implemented.
                div {
                    style: format!(
                        "width: {sz}px; height: {sz}px; border-radius: {r}px; \
                         background: {bg};",
                        sz = ICON_SIZE_LG,
                        r  = RADIUS_SM,
                        bg = "#3D7EFF",
                    ),
                }
            }

            // ── Document title (center, drag region placeholder) ───────────────
            // COMPAT(dioxus-native): Window drag region requires a winit platform
            // event. Implemented as a flex spacer for now; dragging uses OS default
            // behavior until blitz-shell exposes a drag-region API.
            // TODO(title-edit): Make the title field inline-editable (future pass).
            span {
                style: format!(
                    "flex: 1; font-size: {size}px; font-weight: {weight}; \
                     color: {fg}; text-align: center; \
                     overflow: hidden;",
                    // COMPAT(dioxus-native): text-overflow: ellipsis is unconfirmed —
                    // verify at runtime.
                    size   = FONT_SIZE_BODY,
                    weight = FONT_WEIGHT_SEMIBOLD,
                    fg     = COLOR_TEXT_ON_CHROME,
                ),
                "{title_text}"
            }

            // ── Right slot: collaborator badge ────────────────────────────────
            if props.collaborator_count > 0 {
                span {
                    style: format!(
                        "font-size: {size}px; color: {fg}; flex-shrink: 0;",
                        size = FONT_SIZE_LABEL,
                        fg   = COLOR_TEXT_ACCENT,
                    ),
                    "{props.collaborator_label}"
                }
            }

            // App name in top-right corner when a document is open. Hidden at
            // Compact width — the app icon already identifies the app, and the
            // narrow title bar needs the space for the document title.
            if props.document_title.is_some() && !compact {
                span {
                    style: format!(
                        "font-size: {size}px; font-weight: {weight}; \
                         color: {fg}; flex-shrink: 0;",
                        size   = FONT_SIZE_LABEL,
                        weight = FONT_WEIGHT_BOLD,
                        fg     = COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    "{props.app_name}"
                }
            }

            // Spacer for symmetry on right when no right-slot content
            div { style: format!("width: {w}px; flex-shrink: 0;", w = SPACE_1) }
        }
    }
}

// ── Props ─────────────────────────────────────────────────────────────────────

/// Props for [`AtTitleBar`].
#[derive(Props, Clone, PartialEq)]
pub struct AtTitleBarProps {
    /// Current document title. `None` = show only `app_name` in the center.
    pub document_title: Option<String>,

    /// When `true`, prepend `"• "` to the document title to signal unsaved changes.
    pub is_dirty: bool,

    /// Application name shown when no document is open, and in the right slot
    /// when a document title is present.
    pub app_name: &'static str,

    /// Number of active remote collaborators. `0` = hide the collaborator badge.
    pub collaborator_count: u32,

    /// Pre-formatted collaborator label, e.g. `"2 connected"`.
    /// Only rendered when `collaborator_count > 0`.
    pub collaborator_label: String,

    /// Host platform — controls title bar height and left-padding.
    pub platform: Platform,

    /// Callback invoked when the user clicks the app icon area.
    pub on_icon_press: EventHandler<()>,
}
