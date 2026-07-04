// SPDX-License-Identifier: Apache-2.0

//! The list-styles list for the style panel's left column (Spec 05 M6 list
//! family). Mirrors `char_browser` — a non-inheriting browse-and-inspect
//! surface, kept in its own module to hold `mod.rs` under the ceiling.
//!
//! Selecting a list style writes its id into `editing_list_style`, which the
//! panel reads to show that style's per-level rows read-only (§9). List styles
//! are flat (no inheritance), so this is a plain sorted list, not a tree.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

/// Renders the "List styles" heading and one button per list style (empty when
/// the document has none). `list_selected` highlights the active id.
pub(super) fn list_list_section(
    list_list: Vec<(String, String)>,
    list_selected: Option<String>,
    mut editing_list_style: Signal<Option<String>>,
) -> Element {
    if list_list.is_empty() {
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
            { fl!("style-list-family-heading") }
        }
        for (id, display) in list_list.into_iter() {
            {
                let is_sel = list_selected.as_deref() == Some(id.as_str());
                let id_cap = id.clone();
                rsx! {
                    button {
                        key: "list-{id}",
                        style: format!(
                            "text-align: left; padding: {p}px {p2}px; border-radius: 3px; \
                             border: 1px solid {border}; cursor: pointer; font-family: {ff}; \
                             font-size: {fs}px; background: {bg}; color: {fg};",
                            p = tokens::SPACE_1,
                            p2 = tokens::SPACE_2,
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
                        onclick: move |_| editing_list_style.set(Some(id_cap.clone())),
                        "{display}"
                    }
                }
            }
        }
    }
}
