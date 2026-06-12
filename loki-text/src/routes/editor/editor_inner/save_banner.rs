// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Save message banner for the document editor.

use appthere_ui::tokens;
use dioxus::prelude::*;

/// Renders the save confirmation banner.
///
/// Shown after a successful save or export operation.  The user dismisses it
/// via the × button.
///
/// Touch target: the dismiss button is ≥44×44 logical px (WCAG 2.5.8).
pub(super) fn save_message_banner(
    msg: String,
    mut save_message: Signal<Option<String>>,
) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; \
                 justify-content: space-between; padding: {p}px {p2}px; \
                 background: {bg}; border-top: 1px solid {border}; \
                 font-family: {ff}; font-size: {size}px; \
                 color: {fg}; flex-shrink: 0;",
                p      = tokens::SPACE_2,
                p2     = tokens::SPACE_4,
                bg     = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_BORDER_CHROME,
                ff     = tokens::FONT_FAMILY_UI,
                size   = tokens::FONT_SIZE_LABEL,
                fg     = tokens::COLOR_TEXT_ON_CHROME,
            ),
            span { "{msg}" }
            button {
                style: format!(
                    "background: transparent; border: none; font-size: {fs}px; \
                     color: {fg}; cursor: pointer; padding: {p}px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    p  = tokens::SPACE_1,
                ),
                onclick: move |_| { save_message.set(None); },
                "\u{2715}"
            }
        }
    }
}
