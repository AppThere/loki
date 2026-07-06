// SPDX-License-Identifier: Apache-2.0

//! `AtRibbon` — the shell ribbon component.
//!
//! The ribbon sits between the canvas [`Outlet`] and [`AtStatusBar`] in the
//! editor layout.  It consists of two rows:
//!
//! 1. [`AtRibbonTabStrip`] — the row of tab labels (Home, Insert, …).
//! 2. [`AtRibbonContent`] — the scrollable button area for the active tab.
//!
//! Application-specific button content is injected via the `tab_content` slot.
//! [`AtRibbonGroup`] provides the standard clustered-button layout with
//! dividers and optional group labels.
//!
//! # Layout position
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  AtTabBar             (flex-shrink: 0)  │
//! ├─────────────────────────────────────────┤
//! │  Outlet (Home or Editor)  (flex: 1)     │
//! ├─────────────────────────────────────────┤
//! │  AtRibbon             (flex-shrink: 0)  │
//! │    AtRibbonTabStrip   (36 px)           │
//! │    AtRibbonContent    (60 px)           │
//! ├─────────────────────────────────────────┤
//! │  AtStatusBar          (flex-shrink: 0)  │
//! └─────────────────────────────────────────┘
//! ```

pub mod button;
pub mod content_row;
pub mod group;
pub mod groups;
pub mod select;
pub mod tab_strip;

pub use button::AtRibbonIconButton;
pub use content_row::AtRibbonContent;
pub use group::{AtRibbonGroup, AtRibbonGroupProps};
pub use groups::{AtRibbonGroups, AtRibbonGroupsProps, RibbonGroupSpec};
pub use select::AtRibbonSelect;
pub use tab_strip::AtRibbonTabStrip;

use dioxus::prelude::*;

use crate::tokens;

// ── Public types ─────────────────────────────────────────────────────────────

/// Identifies a ribbon tab by index.
///
/// Index 0 is always the first core tab (e.g. Home).  Contextual tabs appear
/// after core tabs when relevant content is selected.
pub type RibbonTabIndex = usize;

/// Describes a single ribbon tab label.
#[derive(Clone, PartialEq)]
pub struct RibbonTabDesc {
    /// Short display label shown in the tab strip (e.g. "Home", "Insert").
    pub label: String,
    /// Whether this is a contextual tab (appears only when relevant content
    /// is selected).  Contextual tabs render in amber (`COLOR_CONTEXTUAL_TAB`).
    pub is_contextual: bool,
    /// ARIA label for the tab.  If `None`, `label` is used.
    pub aria_label: Option<String>,
}

// ── AtRibbon ─────────────────────────────────────────────────────────────────

/// Shell ribbon component — tab strip + optional content row.
///
/// Positioned between the canvas area and [`AtStatusBar`] in the editor.
/// Fires `on_tab_select` when a tab label is clicked.  The active tab's button
/// content is provided by the caller via `tab_content`.
///
/// When `collapsed` is `true` the content row is hidden and only the tab strip
/// remains visible, saving screen space on narrow / landscape-phone viewports.
///
/// # Touch target
///
/// This component is a structural container.  The tab strip height is 36 px
/// (see [`AtRibbonTabStrip`] for the WCAG 2.5.8 note).  The content row is
/// 60 px tall, comfortably accommodating 44 × 44 px buttons.
#[component]
pub fn AtRibbon(
    /// All ribbon tabs to display (core tabs first, then contextual).
    tabs: Vec<RibbonTabDesc>,
    /// Index of the currently active ribbon tab.
    active_tab: RibbonTabIndex,
    /// Fired with the clicked tab's index.
    on_tab_select: EventHandler<RibbonTabIndex>,
    /// Content for the active tab's button row.  Pass `rsx! {}` to render
    /// an empty content row (e.g. when no document is open).
    tab_content: Element,
    /// When `true` the content row is hidden; only the tab strip is visible.
    collapsed: bool,
    /// Fired when the user presses the collapse/expand toggle in the tab strip.
    on_toggle_collapse: EventHandler<()>,
    /// Accessible label for the toggle button — should be the translated
    /// "Collapse ribbon" or "Expand ribbon" string from the caller.
    toggle_aria_label: String,
) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex-shrink: 0; width: 100%; \
                 background: {bg}; border-top: 1px solid {border};",
                bg     = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            AtRibbonTabStrip {
                tabs: tabs,
                active_tab: active_tab,
                on_tab_select: move |idx| on_tab_select.call(idx),
                collapsed: collapsed,
                on_toggle_collapse: move |_| on_toggle_collapse.call(()),
                toggle_aria_label: toggle_aria_label,
            }

            if !collapsed {
                AtRibbonContent {
                    {tab_content}
                }
            }
        }
    }
}
