// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Editor route — thin routing shell.
//!
//! Renders [`EditorInner`] with the document `path` prop.  Document switching
//! is handled reactively inside `EditorInner` via `use_memo` — see
//! `editor_inner.rs` for the full design.
//!
//! All editing logic lives in [`editor_inner::EditorInner`].

mod editor_canvas;
mod editor_docked_panels;
mod editor_error_view;
mod editor_font_warning;
mod editor_formatting;
mod editor_inner;
mod editor_insert;
mod editor_insert_panel;
mod editor_keydown;
mod editor_keydown_ctrl;
mod editor_language_panel;
mod editor_layout_task;
mod editor_load;
mod editor_metadata;
mod editor_metadata_panel;
mod editor_path_sync;
mod editor_pointer;
mod editor_publish;
mod editor_responsive;
mod editor_ribbon;
mod editor_ribbon_insert;
mod editor_save;
mod editor_save_banner;
mod editor_save_callbacks;
mod editor_scrollbar;
mod editor_spell;
mod editor_spell_panel;
mod editor_state;
mod editor_style;
mod editor_style_catalog;
mod editor_style_editor;
mod style_char_inspector;
mod style_impact;
mod style_inspector;

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
