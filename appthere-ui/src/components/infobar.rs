// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `AtInfobar` — a non-modal, full-width notice strip shown under the ribbon.
//!
//! Used for passive, document-scoped security/status notices that must **not**
//! interrupt the user with a modal (prompt fatigue trains users to click
//! through). The first consumer is the "this document contains macros — macros
//! are disabled" notice (macro spec §9.1): opening a macro-carrying document is
//! never blocked; the infobar states the fact and offers an opt-in action.
//!
//! It is an ordinary in-flow block (not an overlay), so it simply sits between
//! the ribbon and the canvas and pushes content down — no positioned-ancestor
//! contract like [`super::confirm_dialog`].
//!
//! Touch target: the action and dismiss controls are at least 44×44 logical
//! pixels (`min-height: TOUCH_MIN`, padded width) per WCAG 2.5.8.

use dioxus::prelude::*;

use crate::tokens::colors::{
    COLOR_CONTEXTUAL_TAB, COLOR_SURFACE_1, COLOR_TEXT_ON_CHROME, COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::spacing::{RADIUS_SM, SPACE_1, SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::{FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_WEIGHT_SEMIBOLD};

/// Props for [`AtInfobar`]. All display strings are props (i18n-agnostic).
#[derive(Props, Clone, PartialEq)]
pub struct AtInfobarProps {
    /// The notice text (e.g. "This document contains macros. Macros are
    /// disabled.").
    pub message: String,
    /// Label for the optional primary action (e.g. "Enable options…"). When
    /// `None`, no action button is shown.
    #[props(default)]
    pub action_label: Option<String>,
    /// Invoked when the action button is clicked. Ignored without
    /// `action_label`.
    #[props(default)]
    pub on_action: Option<EventHandler<()>>,
    /// Label for an optional secondary action (e.g. "View macros…"), shown left
    /// of the primary action. When `None`, no secondary button is shown.
    #[props(default)]
    pub secondary_label: Option<String>,
    /// Invoked when the secondary action is clicked. Ignored without
    /// `secondary_label`.
    #[props(default)]
    pub on_secondary: Option<EventHandler<()>>,
    /// Accessible label for the dismiss (×) control. When `None`, the infobar
    /// is not dismissable and no × is shown.
    #[props(default)]
    pub dismiss_label: Option<String>,
    /// Invoked when the dismiss control is clicked.
    #[props(default)]
    pub on_dismiss: Option<EventHandler<()>>,
}

/// A passive warning strip. See the module docs for placement (in-flow, under
/// the ribbon) and the 44×44 px touch-target guarantee on its controls.
#[component]
pub fn AtInfobar(props: AtInfobarProps) -> Element {
    let button_style = format!(
        "min-height: {th}px; box-sizing: border-box; padding: {py}px {px}px; \
         border-radius: {r}px; font-family: {font}; font-size: {fs}px; \
         font-weight: {fw}; background: transparent; border: 1px solid {accent}; \
         color: {accent}; cursor: pointer; display: flex; align-items: center;",
        th = TOUCH_MIN,
        py = SPACE_1,
        px = SPACE_3,
        r = RADIUS_SM,
        font = FONT_FAMILY_UI,
        fs = FONT_SIZE_BODY,
        fw = FONT_WEIGHT_SEMIBOLD,
        accent = COLOR_CONTEXTUAL_TAB,
    );

    rsx! {
        div {
            role: "status",
            style: format!(
                "display: flex; align-items: center; gap: {gap}px; width: 100%; \
                 box-sizing: border-box; padding: {py}px {px}px; \
                 background: {bg}; border-bottom: 1px solid {accent}; \
                 border-left: 3px solid {accent}; \
                 font-family: {font}; font-size: {fs}px; color: {fg};",
                gap = SPACE_2,
                py = SPACE_2,
                px = SPACE_3,
                bg = COLOR_SURFACE_1,
                accent = COLOR_CONTEXTUAL_TAB,
                font = FONT_FAMILY_UI,
                fs = FONT_SIZE_BODY,
                fg = COLOR_TEXT_ON_CHROME,
            ),

            // Warning glyph.
            span {
                style: format!("color: {accent}; font-weight: {fw};", accent = COLOR_CONTEXTUAL_TAB, fw = FONT_WEIGHT_SEMIBOLD),
                "aria-hidden": "true",
                "⚠"
            }

            // Message — takes the remaining width.
            span {
                style: format!("flex: 1; color: {fg};", fg = COLOR_TEXT_ON_CHROME),
                {props.message.clone()}
            }

            // Optional secondary action (left of the primary).
            if let Some(label) = props.secondary_label.clone() {
                button {
                    style: button_style.clone(),
                    onclick: move |_| {
                        if let Some(cb) = &props.on_secondary {
                            cb.call(());
                        }
                    },
                    {label}
                }
            }

            // Optional primary action.
            if let Some(label) = props.action_label.clone() {
                button {
                    style: button_style.clone(),
                    onclick: move |_| {
                        if let Some(cb) = &props.on_action {
                            cb.call(());
                        }
                    },
                    {label}
                }
            }

            // Optional dismiss control.
            if let Some(aria) = props.dismiss_label.clone() {
                button {
                    "aria-label": aria,
                    style: format!(
                        "min-width: {th}px; min-height: {th}px; box-sizing: border-box; \
                         background: transparent; border: none; cursor: pointer; \
                         color: {fg}; font-size: {fs}px; display: flex; \
                         align-items: center; justify-content: center;",
                        th = TOUCH_MIN,
                        fg = COLOR_TEXT_ON_CHROME_SECONDARY,
                        fs = FONT_SIZE_BODY,
                    ),
                    onclick: move |_| {
                        if let Some(cb) = &props.on_dismiss {
                            cb.call(());
                        }
                    },
                    "×"
                }
            }
        }
    }
}
