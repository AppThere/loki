// SPDX-License-Identifier: Apache-2.0

//! Colour-picking controls: [`AtColorPickerTrigger`] (a ribbon button with a
//! live colour-indicator bar) and [`AtColorPickerPanel`] (preset swatches, the
//! caller's recent colours, and an optional custom-colour entry — Hex / RGB /
//! HSL / HSV / CMYK).
//!
//! The two are separate components because the ribbon content row is a scroll
//! container (`overflow-x: auto; overflow-y: hidden`).
//! COMPAT(dioxus-native): an absolutely-positioned popover inside that row is
//! clipped and never paints, so the panel must be hosted *outside* the ribbon
//! — apps dock it in the flex column above the ribbon, the same posture as
//! their other panels — while the trigger stays in the ribbon group and
//! toggles the panel's open state.
//!
//! The picker is value-agnostic: swatch `value`s are opaque strings the caller
//! interprets (hex for text colour, named variants for highlight), and "recent
//! colours" are supplied — and recorded — by the caller via `on_pick`.

use dioxus::prelude::*;

use super::ribbon::AtRibbonIconButton;
use crate::tokens;

pub mod convert;
mod custom;

use custom::CustomColorSection;

/// Swatches per panel row (6 × 44 px buttons keeps the grid compact).
const SWATCHES_PER_ROW: usize = 6;

/// One selectable colour: the opaque `value` reported on pick, the CSS `fill`
/// shown in the swatch square, and its accessible name.
#[derive(Clone, PartialEq)]
pub struct AtColorSwatch {
    /// Opaque value reported to `on_pick` (e.g. a hex string or variant name).
    pub value: String,
    /// CSS colour painted in the swatch square.
    pub fill: String,
    /// Accessible name of the swatch button.
    pub aria_label: String,
}

/// Translated prose labels for the panel's sections and actions.
#[derive(Clone, PartialEq)]
pub struct AtColorPickerLabels {
    /// Panel heading (e.g. "Font colour").
    pub title: String,
    /// Accessible name of the panel's close button.
    pub close: String,
    /// The "clear / automatic / none" action label.
    pub clear: String,
    /// Heading of the recent-colours section.
    pub recent_heading: String,
    /// Heading of the custom-colour section.
    pub custom_heading: String,
    /// Apply button label of the custom-colour section.
    pub apply: String,
}

/// A small filled square used inside swatch buttons.
fn square(fill: &str) -> Element {
    rsx! {
        div {
            style: format!(
                "width: 18px; height: 18px; border-radius: {r}px; \
                 background: {fill}; border: 1px solid rgba(0,0,0,0.25);",
                r = tokens::RADIUS_SM,
            ),
        }
    }
}

/// Ribbon trigger button for a colour picker: the caller's glyph stacked above
/// an indicator bar showing the currently applied colour.
///
/// # Touch target
///
/// An [`AtRibbonIconButton`] — 44×44 logical pixels (WCAG 2.5.8).
#[component]
pub fn AtColorPickerTrigger(
    /// Accessible name of the trigger button.
    aria_label: String,
    /// CSS colour shown in the indicator bar; `None` renders the "no override"
    /// outline.
    current_fill: Option<String>,
    /// Whether the associated panel is open (renders the button active).
    is_open: bool,
    /// Fired on click; the caller toggles its panel-open state.
    on_toggle: EventHandler<()>,
    /// Trigger glyph (e.g. an icon), stacked above the indicator bar.
    children: Element,
) -> Element {
    let indicator = match &current_fill {
        Some(fill) => {
            format!("width: 18px; height: 4px; border-radius: 1px; background: {fill};")
        }
        None => format!(
            "width: 18px; height: 4px; border-radius: 1px; background: transparent; \
             border: 1px solid {};",
            tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
        ),
    };
    rsx! {
        AtRibbonIconButton {
            aria_label: aria_label,
            is_active: is_open,
            is_disabled: false,
            on_click: move |_| on_toggle.call(()),
            div {
                style: "display: flex; flex-direction: column; align-items: center; gap: 2px;",
                {children}
                div { style: "{indicator}" }
            }
        }
    }
}

/// The colour-picker panel: clear action + preset swatch grid, the caller's
/// recent colours, and (optionally) the custom-colour entry. In-flow — the app
/// docks it above the ribbon.
///
/// # Touch target
///
/// Every swatch is an [`AtRibbonIconButton`] (44×44 logical pixels); the clear
/// row and close button carry ≥44 px hit areas via `min-height`/padding
/// (WCAG 2.5.8).
#[component]
pub fn AtColorPickerPanel(
    /// The currently applied value (drives the active swatch highlight).
    current_value: Option<String>,
    /// Preset swatches, rendered [`SWATCHES_PER_ROW`] per row.
    swatches: Vec<AtColorSwatch>,
    /// The caller's recent colours, most recent first (section hidden if empty).
    recent: Vec<AtColorSwatch>,
    /// Whether to render the custom-colour entry section.
    show_custom: bool,
    /// Translated section/action labels.
    labels: AtColorPickerLabels,
    /// Fired with `Some(value)` for a pick and `None` for clear. Recording the
    /// pick into the recent list is the caller's job.
    on_pick: EventHandler<Option<String>>,
    /// Fired by the close button.
    on_close: EventHandler<()>,
) -> Element {
    let heading_style = format!(
        "font-size: {fs}px; color: {fg};",
        fs = tokens::FONT_SIZE_XS,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
    );
    let text_btn = format!(
        "display: flex; flex-direction: row; align-items: center; gap: {gap}px; \
         padding: {pv}px {ph}px; min-height: {touch}px; background: transparent; \
         border: 1px solid {border}; border-radius: {r}px; cursor: pointer; \
         color: {fg}; font-family: {ff}; font-size: {fs}px;",
        gap = tokens::SPACE_2,
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_2,
        touch = tokens::TOUCH_MIN,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        ff = tokens::FONT_FAMILY_UI,
        fs = tokens::FONT_SIZE_LABEL,
    );

    rsx! {
        div {
            // In-flow band above the ribbon, styled like the app's other panels.
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px; \
                 padding: {pv}px {ph}px; background: {bg}; \
                 border-top: 1px solid {border}; border-bottom: 1px solid {border}; \
                 font-family: {ff}; color: {fg}; flex-shrink: 0;",
                gap = tokens::SPACE_2,
                pv = tokens::SPACE_2,
                ph = tokens::SPACE_4,
                bg = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_BORDER_CHROME,
                ff = tokens::FONT_FAMILY_UI,
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),

            // Header: title + close.
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                span {
                    style: format!("font-weight: bold; font-size: {}px;", tokens::FONT_SIZE_BODY),
                    "{labels.title}"
                }
                div { style: "flex: 1;" }
                button {
                    style: "{text_btn}",
                    aria_label: labels.close.clone(),
                    onclick: move |_| on_close.call(()),
                    "\u{2715}"
                }
            }

            // Sections side by side; each column is narrow.
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: flex-start; gap: {}px;",
                    tokens::SPACE_6,
                ),

                // Clear + preset grid.
                div {
                    style: format!(
                        "display: flex; flex-direction: column; gap: {}px;",
                        tokens::SPACE_2,
                    ),
                    button {
                        style: "{text_btn}",
                        onclick: move |_| on_pick.call(None),
                        {square("transparent")}
                        "{labels.clear}"
                    }
                    {swatch_rows(&swatches, &current_value, on_pick)}
                }

                // Recent colours.
                if !recent.is_empty() {
                    div {
                        style: format!(
                            "display: flex; flex-direction: column; gap: {}px;",
                            tokens::SPACE_2,
                        ),
                        span { style: "{heading_style}", "{labels.recent_heading}" }
                        {swatch_rows(&recent, &current_value, on_pick)}
                    }
                }

                // Custom entry.
                if show_custom {
                    CustomColorSection {
                        heading: labels.custom_heading.clone(),
                        apply_label: labels.apply.clone(),
                        on_apply: move |hex: String| on_pick.call(Some(hex)),
                    }
                }
            }
        }
    }
}

/// Renders `swatches` as rows of [`SWATCHES_PER_ROW`] swatch buttons.
fn swatch_rows(
    swatches: &[AtColorSwatch],
    current_value: &Option<String>,
    pick: EventHandler<Option<String>>,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: column; gap: 2px;",
            for (row_idx, row) in swatches.chunks(SWATCHES_PER_ROW).enumerate() {
                div {
                    key: "{row_idx}",
                    style: "display: flex; flex-direction: row; gap: 2px;",
                    for sw in row.iter().cloned() {
                        AtRibbonIconButton {
                            key: "{sw.value}",
                            aria_label: sw.aria_label.clone(),
                            is_active: current_value.as_deref() == Some(sw.value.as_str()),
                            is_disabled: false,
                            on_click: {
                                let value = sw.value.clone();
                                move |_| pick.call(Some(value.clone()))
                            },
                            {square(&sw.fill)}
                        }
                    }
                }
            }
        }
    }
}
