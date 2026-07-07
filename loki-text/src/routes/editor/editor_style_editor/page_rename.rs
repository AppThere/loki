// SPDX-License-Identifier: Apache-2.0

//! The page-style **rename** field (Spec 05 M6 page family — LibreOffice-style
//! named page styles).
//!
//! A tiny component so it can own its own draft signal (ADR-0013): the text input
//! edits a local copy of the name, and the Rename button hands the new name to
//! the parent's `on_rename` callback, which runs the `rename_page_style`
//! mutation. Key this by the current name so switching page styles reseeds the
//! draft.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::form_font::{input_style, label_style};

/// A labelled text input + Rename button for the selected page style's name.
///
/// # Touch target
///
/// The Rename button is a text button; its height follows the shared input
/// height (24 px) — on touch builds the base font scale lifts it toward the
/// 44 px WCAG 2.5.8 minimum, consistent with the other style-panel controls.
#[component]
pub(super) fn PageRenameField(name: String, on_rename: EventHandler<String>) -> Element {
    let mut draft = use_signal(|| name.clone());
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 6px; margin-bottom: 6px;",
            span {
                style: format!("{} min-width: 64px;", label_style()),
                { fl!("style-page-name-label") }
            }
            input {
                r#type: "text",
                value: "{draft}",
                oninput: move |evt| draft.set(evt.value()),
                style: input_style("flex: 1"),
            }
            button {
                style: format!(
                    "padding: 2px 8px; border-radius: 3px; border: 1px solid {border}; \
                     cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                     background: {bg}; color: {fg};",
                    border = tokens::COLOR_TAB_ACTIVE_INDICATOR,
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_LABEL,
                    bg = tokens::COLOR_SURFACE_3,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
                onclick: move |_| on_rename.call(draft.read().clone()),
                { fl!("style-page-rename") }
            }
        }
    }
}
