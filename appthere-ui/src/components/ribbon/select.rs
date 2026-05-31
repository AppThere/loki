// SPDX-License-Identifier: Apache-2.0

//! [`AtRibbonSelect`] — a style-name selector button for the ribbon.
//!
//! Renders the current value as a labelled button in the ribbon content row.
//! Pressing it fires `on_open` so the caller can display a full-height picker
//! panel above the ribbon (the only layout-safe approach in Blitz, where
//! `position: absolute` is unsupported — see COMPAT note below).
//!
//! # Touch target
//!
//! The button is at least 44 × 44 logical pixels (WCAG 2.5.8).

use dioxus::prelude::*;

use crate::tokens::{
    colors::{
        COLOR_BORDER_CHROME, COLOR_SURFACE_3, COLOR_TAB_ACTIVE_INDICATOR, COLOR_TEXT_ON_CHROME,
        COLOR_TEXT_ON_CHROME_SECONDARY,
    },
    spacing::{SPACE_2, TOUCH_MIN},
    typography::{FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_LABEL, FONT_WEIGHT_REGULAR},
};

/// Props for [`AtRibbonSelect`].
#[derive(Props, Clone, PartialEq)]
pub struct AtRibbonSelectProps {
    /// The currently active value shown in the button label.
    pub value: String,
    /// ARIA label for the button (e.g. "Paragraph style").
    pub aria_label: String,
    /// Whether the picker panel is currently open (controls active styling).
    pub is_open: bool,
    /// Fired when the user presses the button to open or close the picker.
    pub on_open: EventHandler<()>,
}

/// Ribbon select button — shows the current paragraph style name and fires
/// `on_open` when the user activates it.
///
/// # COMPAT(dioxus-native)
///
/// A floating dropdown overlay requires `position: absolute`, which is
/// confirmed unsupported in current Blitz. The caller is responsible for
/// rendering the option list as an inline panel in the editor layout (outside
/// the ribbon's `overflow-y: hidden` content row).
///
/// # Touch target
///
/// Button height is clamped to at least `TOUCH_MIN` (44 px) via `min-height`.
#[component]
pub fn AtRibbonSelect(props: AtRibbonSelectProps) -> Element {
    let border_color = if props.is_open {
        COLOR_TAB_ACTIVE_INDICATOR
    } else {
        COLOR_BORDER_CHROME
    };
    let bg_color = if props.is_open {
        COLOR_SURFACE_3
    } else {
        "transparent"
    };

    rsx! {
        button {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; gap: {gap}px; \
                 width: {w}px; min-height: {h}px; padding: 0 {p}px; \
                 background: {bg}; border: 1px solid {border}; border-radius: 4px; \
                 font-family: {ff}; font-size: {fs}px; font-weight: {fw}; \
                 color: {fg}; cursor: pointer; flex-shrink: 0;",
                gap    = SPACE_2,
                w      = 180,
                h      = TOUCH_MIN,
                p      = SPACE_2,
                bg     = bg_color,
                border = border_color,
                ff     = FONT_FAMILY_UI,
                fs     = FONT_SIZE_BODY,
                fw     = FONT_WEIGHT_REGULAR,
                fg     = COLOR_TEXT_ON_CHROME,
            ),
            aria_label: props.aria_label.clone(),
            onclick: move |_| props.on_open.call(()),

            span {
                style: format!(
                    "flex: 1; min-width: 0; overflow: hidden; \
                     font-family: {ff}; font-size: {fs}px; color: {fg};",
                    ff = FONT_FAMILY_UI,
                    fs = FONT_SIZE_BODY,
                    fg = COLOR_TEXT_ON_CHROME,
                ),
                // COMPAT(dioxus-native): text-overflow: ellipsis unconfirmed — omitted.
                "{props.value}"
            }

            span {
                style: format!(
                    "font-size: {fs}px; color: {fg}; flex-shrink: 0;",
                    fs = FONT_SIZE_LABEL,
                    fg = COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                if props.is_open { "▴" } else { "▾" }
            }
        }
    }
}
