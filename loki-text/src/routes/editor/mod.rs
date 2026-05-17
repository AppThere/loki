// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Editor route — thin routing shell.
//!
//! Renders [`EditorInner`] with the document `path` prop.  Document switching
//! is handled reactively inside `EditorInner` via `use_memo` — see
//! `editor_inner.rs` for the full design.
//!
//! All editing logic lives in [`editor_inner::EditorInner`].

mod editor_canvas;
mod editor_error_view;
mod editor_formatting;
mod editor_inner;
mod editor_keydown;
mod editor_keydown_ctrl;
mod editor_load;
mod editor_path_sync;
mod editor_pointer;
mod editor_ribbon;
mod editor_state;

use dioxus::prelude::*;
use editor_inner::EditorInner;

// EditorMode removed — the editor is always in edit mode when a document is
// open. Distraction-free reading is handled by the View ribbon tab (future
// pass), not by a separate mode.

/// Editor route component.
///
/// Intentionally thin — all editing logic lives in [`EditorInner`].
#[component]
pub fn Editor(path: String) -> Element {
    rsx! {
        // Note: key: "{path}" is intentionally omitted. In Dioxus 0.7, `key` on a
        // single non-list component is not processed by the diffing engine and does
        // not force remount. Document switching is handled reactively via use_memo
        // inside EditorInner — see editor_inner.rs.
        EditorInner {
            path: path,
        }
    }
}
