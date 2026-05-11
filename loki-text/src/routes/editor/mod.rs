// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Editor route — thin routing shell.
//!
//! Renders [`EditorInner`] with a `key` prop equal to the document path.
//! When the path changes (tab switch), Dioxus unmounts the old `EditorInner`
//! and mounts a fresh one, ensuring all hook state (document load, GPU
//! surface, Loro bridge, cursor) is cleanly initialised for the new document.
//!
//! All editing logic lives in [`editor_inner::EditorInner`].

mod editor_error_view;
mod editor_inner;
mod editor_keydown;
mod editor_load;
mod editor_pointer;
mod editor_state;

use dioxus::prelude::*;
use editor_inner::EditorInner;

/// Editor view mode toggle.
#[derive(Clone, PartialEq, Copy)]
pub enum EditorMode {
    Reading,
    Editing,
}

/// Editor route component.
///
/// Intentionally thin — all editing logic lives in [`EditorInner`].
/// The `key` attribute on `EditorInner` is the mechanism that fixes stale
/// Vello scenes on tab switch: a changed key forces a full remount, giving
/// the new document clean [`use_hook`] and [`use_resource`] state.
#[component]
pub fn Editor(path: String) -> Element {
    rsx! {
        EditorInner {
            key: "{path}",
            path: path,
        }
    }
}
