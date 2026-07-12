// SPDX-License-Identifier: Apache-2.0

//! [`AtRibbonTabStrip`] and [`AtRibbonTab`] — the horizontal row of ribbon
//! tab labels (Home, Insert, Format, …).
//!
//! Contextual tabs (e.g. Table, Image) render in amber when the relevant
//! content is selected.

use dioxus::prelude::*;

use crate::components::ribbon::RibbonTabDesc;
use crate::components::ribbon::RibbonTabIndex;
use crate::responsive::{use_breakpoint, Breakpoint};
use crate::tokens;
use crate::tokens::FONT_FAMILY_UI;

/// The tab-strip height for a breakpoint (Spec 04 M6, R-14): the touch-first
/// Compact class gets full [`tokens::TOUCH_MIN`] targets (WCAG 2.5.8);
/// pointer-first classes keep the desktop-density
/// [`tokens::RIBBON_TAB_STRIP_HEIGHT`], matching the shell tab-bar convention.
#[must_use]
pub fn tab_strip_height(breakpoint: Breakpoint) -> f32 {
    if breakpoint == Breakpoint::Compact {
        tokens::TOUCH_MIN
    } else {
        tokens::RIBBON_TAB_STRIP_HEIGHT
    }
}

// ── AtRibbonTabStrip ──────────────────────────────────────────────────────────

/// Horizontally scrollable row of ribbon tab labels.
///
/// Renders one [`AtRibbonTab`] per entry in `tabs`.  Fires `on_tab_select`
/// with the selected index when a tab label is clicked.
///
/// # Touch target
///
/// Individual [`AtRibbonTab`] components fill the strip height. At the
/// touch-first Compact breakpoint the strip is [`tokens::TOUCH_MIN`] (44 px)
/// tall, meeting WCAG 2.5.8 (Spec 04 M6, R-14); pointer-first breakpoints
/// keep the desktop-density 36 px ([`tokens::RIBBON_TAB_STRIP_HEIGHT`]),
/// matching the shell tab-bar convention.
#[component]
pub fn AtRibbonTabStrip(
    /// Ordered list of tabs to display (core tabs first, then contextual).
    tabs: Vec<RibbonTabDesc>,
    /// Index of the currently active tab.
    active_tab: RibbonTabIndex,
    /// Fired with the clicked tab's index.
    on_tab_select: EventHandler<RibbonTabIndex>,
    /// Whether the ribbon content row is currently collapsed.
    collapsed: bool,
    /// Fired when the collapse/expand toggle button is pressed.
    on_toggle_collapse: EventHandler<()>,
    /// Accessible label for the collapse/expand toggle button.
    /// Should be `fl!("ribbon-collapse-aria")` or `fl!("ribbon-expand-aria")`.
    toggle_aria_label: String,
) -> Element {
    // Touch posture (R-14): resilient — no responsive context reads Expanded.
    let strip_h = tab_strip_height(use_breakpoint());
    rsx! {
        div {
            role: "tablist",
            style: format!(
                // COMPAT(dioxus-native): overflow-x: auto confirmed working.
                // scrollbar-width: none is unconfirmed — added with COMPAT note.
                "height: {h}px; display: flex; flex-direction: row; \
                 align-items: stretch; overflow-x: auto; \
                 background: {bg}; border-bottom: 1px solid {border}; \
                 flex-shrink: 0;",
                h      = strip_h,
                bg     = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            for (idx, desc) in tabs.iter().enumerate() {
                AtRibbonTab {
                    key: "{idx}",
                    desc: desc.clone(),
                    index: idx,
                    is_active: idx == active_tab,
                    on_select: move |i| on_tab_select.call(i),
                }
            }

            // Spacer pushes the collapse button to the trailing edge.
            div { style: "flex: 1;" }

            // Collapse / expand toggle.
            // Minimum touch target: strip height × min-width 44 px.
            button {
                aria_label: toggle_aria_label,
                style: format!(
                    "min-width: {touch}px; height: {h}px; padding: 0 {p}px; \
                     background: transparent; border: none; cursor: pointer; \
                     color: {fg}; font-size: 11px; flex-shrink: 0; \
                     display: flex; align-items: center; justify-content: center;",
                    touch = tokens::TOUCH_MIN,
                    h     = strip_h,
                    p     = tokens::SPACE_2,
                    fg    = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                onclick: move |_| on_toggle_collapse.call(()),
                // Collapsed → up chevron (click to reveal the content row);
                // expanded → down chevron (click to hide it).
                if collapsed { "▲" } else { "▼" }
            }
        }
    }
}

// ── AtRibbonTab ───────────────────────────────────────────────────────────────

/// A single ribbon tab label button within [`AtRibbonTabStrip`].
///
/// # Touch target
///
/// Fills the strip height — 44 px ([`tokens::TOUCH_MIN`]) at the touch-first
/// Compact breakpoint, 36 px at pointer-first ones (see [`AtRibbonTabStrip`]).
#[component]
fn AtRibbonTab(
    desc: RibbonTabDesc,
    index: RibbonTabIndex,
    is_active: bool,
    on_select: EventHandler<RibbonTabIndex>,
) -> Element {
    let mut hovered = use_signal(|| false);

    let label_color = if desc.is_contextual {
        tokens::COLOR_CONTEXTUAL_TAB
    } else if is_active {
        tokens::COLOR_TEXT_ON_CHROME
    } else {
        tokens::COLOR_TEXT_ON_CHROME_SECONDARY
    };

    let bottom_border = if is_active {
        let indicator = if desc.is_contextual {
            tokens::COLOR_CONTEXTUAL_TAB
        } else {
            tokens::COLOR_TAB_ACTIVE_INDICATOR
        };
        format!("border-bottom: 2px solid {indicator};")
    } else {
        String::new()
    };

    let bg = if hovered() {
        tokens::COLOR_TAB_INACTIVE_HOVER
    } else {
        "transparent"
    };

    let aria_label = desc.aria_label.as_deref().unwrap_or(&desc.label);

    rsx! {
        button {
            role: "tab",
            aria_selected: if is_active { "true" } else { "false" },
            aria_label: aria_label,
            style: format!(
                // Atkinson registration is locked by loki-layout's
                // ui_font_registration test (launch-time blob set).
                "min-width: 64px; padding: 0 {p}px; display: flex; \
                 align-items: center; justify-content: center; \
                 background: {bg}; border: none; cursor: pointer; \
                 font-family: {font}; font-size: {size}px; font-weight: {weight}; \
                 color: {fg}; box-sizing: border-box; {bottom_border}",
                p      = tokens::SPACE_3,
                font   = FONT_FAMILY_UI,
                size   = tokens::FONT_SIZE_BODY,
                weight = tokens::FONT_WEIGHT_MEDIUM,
                fg     = label_color,
                bg     = bg,
            ),
            onmouseenter: move |_| hovered.set(true),
            onmouseleave: move |_| hovered.set(false),
            onclick: move |_| on_select.call(index),
            "{desc.label}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-14: touch-first Compact gets WCAG 2.5.8 targets; pointer-first
    /// classes keep desktop density.
    #[test]
    fn compact_strip_is_touch_sized_and_others_keep_desktop_density() {
        assert_eq!(tab_strip_height(Breakpoint::Compact), tokens::TOUCH_MIN);
        assert_eq!(
            tab_strip_height(Breakpoint::Medium),
            tokens::RIBBON_TAB_STRIP_HEIGHT
        );
        assert_eq!(
            tab_strip_height(Breakpoint::Expanded),
            tokens::RIBBON_TAB_STRIP_HEIGHT
        );
        assert!(tab_strip_height(Breakpoint::Compact) >= 44.0, "WCAG 2.5.8");
    }
}
