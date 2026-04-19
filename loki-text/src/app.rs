// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Root application component.
//!
//! [`App`] is the top-level Dioxus component mounted by [`crate::main`].
//! It wraps the Dioxus router with the [`Route`] enum, wiring up client-side
//! navigation between the Home and Editor screens.
//!
//! Global context providers (theme signals, auth state, etc.) should be added
//! here when they are needed.

use dioxus::prelude::*;

use crate::routes::Route;

/// Root application component.
///
/// Mounts the [`Router`] with the [`Route`] enum as its type parameter.
/// All navigation state lives inside the router; components call
/// [`use_navigator`] to push or replace routes programmatically.
///
/// The outermost `div` applies a CSS reset so Blitz's browser-like defaults
/// (implicit body margin, scrollable root) do not leak into the UI.
#[component]
pub fn App() -> Element {
    rsx! {
        div {
            style: "margin: 0; padding: 0; width: 100vw; height: 100vh; \
                    overflow: hidden; display: flex; flex-direction: column; \
                    box-sizing: border-box;",
            Router::<Route> {}
        }
    }
}
