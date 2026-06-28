// SPDX-License-Identifier: Apache-2.0

//! Transient save-status banner shown below the panels, above the ribbon.
//!
//! Extracted from `editor_inner` (which is over the 300-line ceiling) so adding
//! the spelling panels there is net line-neutral.

use appthere_ui::tokens;
use dioxus::prelude::*;

/// Renders the save-status banner when `save_message` is `Some`, with a close
/// button that clears it.
pub(super) fn save_banner(mut save_message: Signal<Option<String>>) -> Element {
    let Some(msg) = save_message.read().clone() else {
        return rsx! {};
    };
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; \
                 justify-content: space-between; padding: {p}px {p2}px; \
                 background: {bg}; border-top: 1px solid {border}; \
                 font-family: {ff}; font-size: {size}px; \
                 color: {fg}; flex-shrink: 0;",
                p = tokens::SPACE_2,
                p2 = tokens::SPACE_4,
                bg = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_BORDER_CHROME,
                ff = tokens::FONT_FAMILY_UI,
                size = tokens::FONT_SIZE_LABEL,
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            span { "{msg}" }
            button {
                style: format!(
                    "background: transparent; border: none; font-size: {fs}px; \
                     color: {fg}; cursor: pointer; padding: {p}px;",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    p = tokens::SPACE_1,
                ),
                onclick: move |_| { save_message.set(None); },
                "\u{2715}"
            }
        }
    }
}
