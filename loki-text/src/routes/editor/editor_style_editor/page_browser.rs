// SPDX-License-Identifier: Apache-2.0

//! The page-styles list for the style panel's left column (Spec 05 M6 page
//! family, ADR-0012 Decision 2). Mirrors `list_browser` — a non-inheriting
//! browse-and-inspect surface, kept in its own module to hold `mod.rs` under the
//! ceiling.
//!
//! Selecting a page style writes its id into `editing_page_style`, which the
//! panel reads to show that style's geometry rows read-only (§9). Page styles
//! are non-inheriting, so this is a plain list (a flat family, like lists).

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::posture::StylePanelPosture;

/// Renders the "Page styles" heading and one button per page style (empty when
/// the document has none). `page_selected` highlights the active id; `posture`
/// supplies the Compact touch minimum (§11).
pub(super) fn page_list_section(
    page_list: Vec<(String, String)>,
    page_selected: Option<String>,
    mut editing_page_style: Signal<Option<String>>,
    posture: StylePanelPosture,
) -> Element {
    if page_list.is_empty() {
        return rsx! {};
    }
    rsx! {
        div {
            style: format!(
                "font-size: {fs}px; color: {fg}; margin-top: {mt}px; margin-bottom: 2px;",
                fs = tokens::FONT_SIZE_XS,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                mt = tokens::SPACE_3,
            ),
            { fl!("style-page-family-heading") }
        }
        for (id, display) in page_list.into_iter() {
            {
                let is_sel = page_selected.as_deref() == Some(id.as_str());
                let id_cap = id.clone();
                rsx! {
                    button {
                        key: "page-{id}",
                        style: format!(
                            "text-align: left; padding: {p}px {p2}px; border-radius: 3px; {touch} \
                             border: 1px solid {border}; cursor: pointer; font-family: {ff}; \
                             font-size: {fs}px; background: {bg}; color: {fg};",
                            p = tokens::SPACE_1,
                            p2 = tokens::SPACE_2,
                            touch = posture.touch_min_css(),
                            border = if is_sel {
                                tokens::COLOR_TAB_ACTIVE_INDICATOR
                            } else {
                                tokens::COLOR_BORDER_CHROME
                            },
                            ff = tokens::FONT_FAMILY_UI,
                            fs = tokens::FONT_SIZE_LABEL,
                            bg = if is_sel { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        onclick: move |_| editing_page_style.set(Some(id_cap.clone())),
                        "{display}"
                    }
                }
            }
        }
    }
}
