// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Loading placeholder for the document canvas.
//!
//! Extracted from `editor_canvas.rs` to keep that file under the 300-line
//! ceiling.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

/// Blank page placeholder shown while a document is being opened.
///
/// Renders immediately when the editor tab mounts (before the async load
/// resolves), so the user sees a page-shaped surface with an "opening" label
/// instead of an empty canvas while the file is read, imported, and laid out.
pub(super) fn loading_view() -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex: 1; align-items: flex-start; \
                 justify-content: center; width: 100%; padding-top: {gap}px;",
                gap = tokens::SPACE_6,
            ),
            div {
                style: format!(
                    "width: {w}px; height: {h}px; flex-shrink: 0; background: {page}; \
                     border: 1px solid {border}; border-radius: 2px; display: flex; \
                     align-items: center; justify-content: center;",
                    w = tokens::PAGE_WIDTH_PX,
                    h = tokens::PAGE_HEIGHT_PX,
                    page = tokens::CANVAS_PAGE_BG,
                    border = tokens::COLOR_BORDER_CHROME,
                ),
                span {
                    style: format!(
                        "font-family: {ff}; font-size: {fs}px; color: {fg};",
                        ff = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_BODY,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { fl!("editor-document-loading") }
                }
            }
        }
    }
}
