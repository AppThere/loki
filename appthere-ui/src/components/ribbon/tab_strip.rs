// SPDX-License-Identifier: Apache-2.0

//! [`AtRibbonTabStrip`] and [`AtRibbonTab`] — the horizontal row of ribbon
//! tab labels (Home, Insert, Format, …).
//!
//! Contextual tabs (e.g. Table, Image) render in amber when the relevant
//! content is selected.

use dioxus::prelude::*;

use crate::components::ribbon::RibbonTabDesc;
use crate::components::ribbon::RibbonTabIndex;
use crate::tokens;
use crate::tokens::FONT_FAMILY_UI;

// ── AtRibbonTabStrip ──────────────────────────────────────────────────────────

/// Horizontally scrollable row of ribbon tab labels.
///
/// Renders one [`AtRibbonTab`] per entry in `tabs`.  Fires `on_tab_select`
/// with the selected index when a tab label is clicked.
///
/// # Touch target
///
/// Individual [`AtRibbonTab`] components fill the strip height
/// ([`tokens::RIBBON_TAB_STRIP_HEIGHT`] = 36 px), which is below the WCAG
/// 2.5.8 minimum of 44 px.  This matches the existing tab bar convention in
/// the AppThere shell; a future design pass should increase the strip height
/// to 44 px or add invisible padding to meet the requirement.
#[component]
pub fn AtRibbonTabStrip(
    /// Ordered list of tabs to display (core tabs first, then contextual).
    tabs: Vec<RibbonTabDesc>,
    /// Index of the currently active tab.
    active_tab: RibbonTabIndex,
    /// Fired with the clicked tab's index.
    on_tab_select: EventHandler<RibbonTabIndex>,
) -> Element {
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
                h      = tokens::RIBBON_TAB_STRIP_HEIGHT,
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
        }
    }
}

// ── AtRibbonTab ───────────────────────────────────────────────────────────────

/// A single ribbon tab label button within [`AtRibbonTabStrip`].
///
/// # Touch target
///
/// Fills the strip height (36 px) — see [`AtRibbonTabStrip`] for the known
/// WCAG 2.5.8 limitation.
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

    let aria_label = desc.aria_label.unwrap_or(desc.label);

    rsx! {
        button {
            role: "tab",
            aria_selected: if is_active { "true" } else { "false" },
            aria_label: aria_label,
            style: format!(
                // TODO(font): verify Atkinson Hyperlegible Next is registered
                // and loading correctly — ribbon tab labels should not be in system-ui.
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
