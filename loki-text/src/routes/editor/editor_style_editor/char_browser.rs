// SPDX-License-Identifier: Apache-2.0

//! The character-styles list for the style panel's left column (Spec 05 M6
//! character family). Extracted from `mod.rs` to keep it under the ceiling.
//!
//! Selecting a character style writes its id into `editing_char_style` (driving
//! the read-only provenance inspector, §9) **and** seeds `editing_char_draft`
//! from the catalog style, which the panel renders as the editable character
//! form (Spec 05 M6, 4a.3).

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_state::StyleDraft;
use super::super::editor_style_catalog::get_catalog_char_style;
use super::draft::char_style_to_draft;
use super::posture::StylePanelPosture;
use crate::editing::state::DocumentState;

/// Renders the "Character styles" heading and one button per character style
/// (empty when the document has none). `char_selected` highlights the active id;
/// clicking a style selects it for the inspector and seeds the editable draft.
/// `posture` supplies the Compact touch minimum (§11).
pub(super) fn char_list_section(
    doc_state: Arc<Mutex<DocumentState>>,
    char_list: Vec<(String, String)>,
    char_selected: Option<String>,
    mut editing_char_style: Signal<Option<String>>,
    mut editing_char_draft: Signal<Option<StyleDraft>>,
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
                let ds_c = Arc::clone(&doc_state);
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
                        onclick: move |_| {
                            editing_char_style.set(Some(id_cap.clone()));
                            // Seed the editable draft from the selected style.
                            if let Some(s) = get_catalog_char_style(&ds_c, &id_cap) {
                                editing_char_draft.set(Some(char_style_to_draft(&s)));
                            }
                        },
                        "{display}"
                    }
                }
            }
        }
    }
}
