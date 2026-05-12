// SPDX-License-Identifier: Apache-2.0

//! `AtRibbon` — the shell ribbon component.
//!
//! The ribbon sits between the canvas [`Outlet`] and [`AtStatusBar`] in the
//! Shell layout.  It consists of two rows:
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

pub mod content_row;
pub mod group;
pub mod tab_strip;

pub use content_row::AtRibbonContent;
pub use group::{AtRibbonGroup, AtRibbonGroupProps};
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
    pub label: &'static str,
    /// Whether this is a contextual tab (appears only when relevant content
    /// is selected).  Contextual tabs render in amber (`COLOR_CONTEXTUAL_TAB`).
    pub is_contextual: bool,
    /// ARIA label for the tab.  If `None`, `label` is used.
    pub aria_label: Option<&'static str>,
}

// ── AtRibbon ─────────────────────────────────────────────────────────────────

/// Shell ribbon component — tab strip + content row.
///
/// Positioned between the canvas [`Outlet`] and [`AtStatusBar`] in the Shell.
/// Fires `on_tab_select` when a tab label is clicked.  The active tab's button
/// content is provided by the caller via `tab_content`.
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
            }

            AtRibbonContent {
                {tab_content}
            }
        }
    }
}
