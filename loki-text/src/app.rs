// SPDX-License-Identifier: Apache-2.0

//! Root application component.
//!
//! [`App`] is the top-level Dioxus component mounted by [`crate::main`].
//! It injects the [`appthere_ui::AtThemeContext`] and the open-tab signals so
//! all shell components can read the active theme variant and tab list, then
//! wraps the Dioxus router with the [`Route`] enum, wiring up client-side
//! navigation between the Home and Editor screens.
//!
//! ## Root layout reset
//!
//! Blitz's user-agent stylesheet applies `body { margin: 8px; }` by default,
//! matching browser behaviour. The injected [`document::Style`] resets this
//! to zero so the application fills the native window edge-to-edge with no
//! visible gap. Without the reset, the 8px body margin causes the root
//! container's `height: 100vh` to overflow (`100vh + 8px`), making the
//! window vertically scrollable.

use appthere_ui::AtThemeContext;
use dioxus::prelude::*;

use crate::routes::Route;
use crate::tabs::OpenTab;

/// Root application component.
///
/// Injects [`AtThemeContext`] (defaults to `ThemeVariant::Dark`) and two tab
/// signals before any shell component renders, then mounts the [`Router`].
/// All navigation state lives inside the router; components call
/// [`use_navigator`] to push or replace routes programmatically.
#[component]
pub fn App() -> Element {
    // Inject the theme context before any shell component renders.
    provide_context(AtThemeContext::default());

    // Open-document tab list. Index 0 of the Vec = document tab 1 (tab bar
    // index 1, because index 0 is the always-present Home tab).
    let tabs: Signal<Vec<OpenTab>> = use_signal(Vec::new);
    let active_tab: Signal<usize> = use_signal(|| 0usize); // 0 = Home tab

    provide_context(tabs);
    provide_context(active_tab);

    rsx! {
        // Reset the body margin injected by Blitz's user-agent stylesheet so
        // the app fills the native window without an 8px gap on every edge.
        document::Style {
            "
            html, body, main {{
                margin: 0;
                padding: 0;
                overflow: hidden;
                height: 100%;
            }}
            "
        }

        div {
            // Shell owns height: 100vh and the flex column layout.
            // This div simply fills the viewport so the Router has a sized
            // container to work inside.
            style: "margin: 0; padding: 0; width: 100vw; height: 100vh; \
                    overflow: hidden; box-sizing: border-box;",
            Router::<Route> {}
        }
    }
}
