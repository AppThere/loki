// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Editor route — thin routing shell.

mod editor_error_view;
mod editor_inner;
mod editor_load;
mod slide_view;

use dioxus::prelude::*;
use editor_inner::EditorInner;

/// Editor route component.
#[component]
pub fn Editor(path: String) -> Element {
    rsx! {
        EditorInner {
            path: path,
        }
    }
}
