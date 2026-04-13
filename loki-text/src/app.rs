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
#[component]
pub fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}
