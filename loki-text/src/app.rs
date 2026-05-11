// SPDX-License-Identifier: Apache-2.0

//! Root application component.
//!
//! [`App`] is the top-level Dioxus component mounted by [`crate::main`].
//! It injects the [`appthere_ui::AtThemeContext`] so all shell components can
//! read the active theme variant, then wraps the Dioxus router with the
//! [`Route`] enum, wiring up client-side navigation between the Home and
//! Editor screens.
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

/// Root application component.
///
/// Injects [`AtThemeContext`] (defaults to `ThemeVariant::Dark`) before any
/// shell component renders, then mounts the [`Router`].
/// All navigation state lives inside the router; components call
/// [`use_navigator`] to push or replace routes programmatically.
#[component]
pub fn App() -> Element {
    // Inject the theme context before any shell component renders.
    provide_context(AtThemeContext::default());

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
            style: "margin: 0; padding: 0; width: 100vw; height: 100vh; \
                    overflow: hidden; display: flex; flex-direction: column; \
                    box-sizing: border-box;",
            Router::<Route> {}
        }
    }
}
