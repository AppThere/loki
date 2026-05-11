// SPDX-License-Identifier: Apache-2.0

//! Inline load-failure view for the editor route.

use appthere_ui::tokens;
use dioxus::prelude::*;

use crate::routes::Route;

/// Renders an inline load-failure notice with a "Go back" button.
///
/// Shown by [`super::editor_inner::EditorInner`] when the document import
/// pipeline returns an error.
#[component]
pub(super) fn EditorErrorView(message: String) -> Element {
    let navigator = use_navigator();
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; \
                 justify-content: center; align-items: center; \
                 gap: {gap}px;",
                gap = tokens::SPACE_4,
            ),
            span {
                style: format!(
                    "font-size: {size}px; color: {fg};",
                    size = tokens::FONT_SIZE_BODY,
                    fg   = tokens::COLOR_TEXT_PRIMARY,
                ),
                "{message}"
            }
            button {
                style: format!(
                    "padding: {p}px {p2}px; background: {bg}; \
                     border: 1px solid {border}; border-radius: 4px; \
                     font-size: {size}px; cursor: pointer;",
                    p      = tokens::SPACE_2,
                    p2     = tokens::SPACE_4,
                    bg     = tokens::COLOR_SURFACE_PAGE,
                    border = tokens::COLOR_BORDER_DEFAULT,
                    size   = tokens::FONT_SIZE_BODY,
                ),
                onclick: move |_| { navigator.push(Route::Home {}); },
                "Go back"
            }
        }
    }
}
