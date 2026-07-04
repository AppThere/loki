// SPDX-License-Identifier: Apache-2.0

//! The character-styles list for the style panel's left column (Spec 05 M6
//! character family). Extracted from `mod.rs` to keep it under the ceiling.
//!
//! Selecting a character style writes its id into `editing_char_style`, which the
//! panel reads to show that style's read-only provenance inspector (§9). Editing
//! character styles is a later increment — this pass browses and inspects them.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::posture::StylePanelPosture;

/// Renders the "Character styles" heading and one button per character style
/// (empty when the document has none). `char_selected` highlights the active id;
/// `posture` supplies the Compact touch minimum (§11).
pub(super) fn char_list_section(
    char_list: Vec<(String, String)>,
    char_selected: Option<String>,
    mut editing_char_style: Signal<Option<String>>,
    posture: StylePanelPosture,
) -> Element {
    if char_list.is_empty() {
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
            { fl!("style-char-family-heading") }
        }
        for (id, display) in char_list.into_iter() {
            {
                let is_sel = char_selected.as_deref() == Some(id.as_str());
                let id_cap = id.clone();
                rsx! {
                    button {
                        key: "char-{id}",
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
                        onclick: move |_| editing_char_style.set(Some(id_cap.clone())),
                        "{display}"
                    }
                }
            }
        }
    }
}
