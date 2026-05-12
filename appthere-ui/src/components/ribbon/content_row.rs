// SPDX-License-Identifier: Apache-2.0

//! [`AtRibbonContent`] — the horizontally scrollable button area below the
//! ribbon tab strip.
//!
//! This component is purely structural.  The actual ribbon groups and buttons
//! are provided by the caller via the `children` slot.

use dioxus::prelude::*;

use crate::tokens;

/// Horizontally scrollable ribbon content row.
///
/// Renders the fixed-height area below the tab strip that holds
/// [`crate::components::ribbon::AtRibbonGroup`] instances.  Content is
/// provided by the caller via `children`.
///
/// # Touch target
///
/// This component is a structural container.  Individual ribbon buttons placed
/// inside must each meet the 44 × 44 px WCAG 2.5.8 minimum touch target.
/// The row is [`tokens::RIBBON_CONTENT_HEIGHT`] px (60 px) tall, which
/// comfortably accommodates standard button sizes.
#[component]
pub fn AtRibbonContent(
    /// Ribbon groups and buttons to render in the content row.
    children: Element,
) -> Element {
    rsx! {
        div {
            style: format!(
                // COMPAT(dioxus-native): overflow-y: hidden with fixed max-height
                // prevents content from expanding the ribbon vertically.
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: row; align-items: center; \
                 overflow-x: auto; overflow-y: hidden; \
                 background: {bg}; flex-shrink: 0;",
                h  = tokens::RIBBON_CONTENT_HEIGHT,
                bg = tokens::COLOR_SURFACE_2,
            ),
            {children}
        }
    }
}
