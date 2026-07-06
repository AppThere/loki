// SPDX-License-Identifier: Apache-2.0

//! The style panel's left navigation column and the Compact section switcher
//! (Spec 05 M7). Extracted from `mod.rs` so the panel shell stays under the
//! 300-line ceiling and the posture-dependent layout lives in one place.
//!
//! `left_column` renders the inheritance-tree picker, the "+ New" button, and the
//! character/list family lists; every control honours [`StylePanelPosture`]'s
//! touch minimum. `section_switcher` is the Compact-only Edit/Inspect segmented
//! control that chooses which stacked group is visible (§11).

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_state::StyleDraft;
use super::super::editor_style_catalog::{get_catalog_style, new_custom_style_id};
use super::posture::StylePanelPosture;
use super::style_to_draft;
use super::{char_browser, list_browser, tree_nav};
use crate::editing::state::DocumentState;

/// The panel's left navigation column: inheritance tree + "+ New" + family lists.
#[allow(clippy::too_many_arguments)]
pub(super) fn left_column(
    doc_state: Arc<Mutex<DocumentState>>,
    styles: Vec<(String, String, usize)>,
    active_id: String,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    new_style_parent: String,
    char_list: Vec<(String, String)>,
    char_selected: Option<String>,
    editing_char_style: Signal<Option<String>>,
    editing_char_draft: Signal<Option<StyleDraft>>,
    list_list: Vec<(String, String)>,
    list_selected: Option<String>,
    editing_list_style: Signal<Option<String>>,
    posture: StylePanelPosture,
) -> Element {
    let ds_new = Arc::clone(&doc_state);
    let ds_char = Arc::clone(&doc_state);
    rsx! {
        div {
            style: format!(
                "width: {w}; min-width: {w}; overflow-y: auto; \
                 border-right: 1px solid {border}; display: flex; \
                 flex-direction: column; gap: 2px; padding: {p}px;",
                w = posture.section_width(160.0),
                border = tokens::COLOR_BORDER_CHROME,
                p = tokens::SPACE_2,
            ),

            // Inheritance-tree picker (Spec 05 §7). At Compact the full indented
            // tree is impractical, so it degrades to a breadcrumb + drill-down
            // (§11, M7); Expanded/Medium keep the indented tree.
            if posture.stack {
                { tree_nav::compact_tree_nav(Arc::clone(&doc_state), active_id.clone(), editing_style_draft, posture) }
            } else {
                {styles.into_iter().map(|(id, display, depth)| {
                    let is_active = id == active_id;
                    let ds_c = Arc::clone(&doc_state);
                    let id_cap = id.clone();
                    rsx! {
                        button {
                            key: "{id}",
                            style: format!(
                                "text-align: left; padding: {p}px {p2}px; \
                                 padding-left: {indent}px; {touch} \
                                 border-radius: 3px; border: 1px solid {border}; \
                                 cursor: pointer; font-family: {ff}; \
                                 font-size: {fs}px; background: {bg}; color: {fg};",
                                p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                                touch = posture.touch_min_css(),
                                indent = tokens::SPACE_2 + depth as f32 * tokens::SPACE_3,
                                border = if is_active { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                                ff = tokens::FONT_FAMILY_UI,
                                fs = tokens::FONT_SIZE_LABEL,
                                bg = if is_active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            onclick: move |_| {
                                if let Some(s) = get_catalog_style(&ds_c, &id_cap) {
                                    editing_style_draft.set(Some(style_to_draft(&s)));
                                }
                            },
                            "{display}"
                        }
                    }
                })}
            }

            button {
                style: format!(
                    "padding: {p}px {p2}px; border-radius: 3px; margin-top: {mt}px; {touch} \
                     border: 1px solid {border}; cursor: pointer; \
                     font-family: {ff}; font-size: {fs}px; \
                     background: {bg}; color: {fg};",
                    p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                    mt = tokens::SPACE_2,
                    touch = posture.touch_min_css(),
                    border = tokens::COLOR_BORDER_DEFAULT,
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_LABEL,
                    bg = tokens::COLOR_SURFACE_2,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
                aria_label: fl!("ribbon-style-new-aria"),
                onclick: move |_| {
                    let new_id = new_custom_style_id(&ds_new);
                    editing_style_draft.set(Some(StyleDraft {
                        id: new_id.clone(),
                        name: new_id,
                        is_custom: true,
                        // Inherit from the current style; leave props empty so
                        // every value shows as inherited.
                        parent: new_style_parent.clone(),
                        ..StyleDraft::default()
                    }));
                },
                { fl!("editor-style-new") }
            }

            // ── Character styles (§9 character family) ─────────────────────────
            { char_browser::char_list_section(ds_char, char_list, char_selected, editing_char_style, editing_char_draft, posture) }

            // ── List styles (§9 list family, non-inheriting) ───────────────────
            { list_browser::list_list_section(list_list, list_selected, editing_list_style, posture) }
        }
    }
}

/// The Compact-only Edit/Inspect segmented control (§11). At Compact the body
/// stacks, so only one group is shown at a time; this toggles between them.
pub(super) fn section_switcher(mut inspect: Signal<bool>) -> Element {
    let seg = |active: bool| {
        format!(
            "flex: 1; min-height: {touch}px; border: 1px solid {border}; \
             cursor: pointer; font-family: {ff}; font-size: {fs}px; \
             background: {bg}; color: {fg};",
            touch = tokens::TOUCH_MIN,
            border = if active {
                tokens::COLOR_TAB_ACTIVE_INDICATOR
            } else {
                tokens::COLOR_BORDER_CHROME
            },
            ff = tokens::FONT_FAMILY_UI,
            fs = tokens::FONT_SIZE_LABEL,
            bg = if active {
                tokens::COLOR_SURFACE_3
            } else {
                tokens::COLOR_SURFACE_2
            },
            fg = tokens::COLOR_TEXT_ON_CHROME,
        )
    };
    let showing_inspect = *inspect.read();
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; gap: {gap}px; \
                 flex-shrink: 0; padding: {p}px;",
                gap = tokens::SPACE_1,
                p = tokens::SPACE_2,
            ),
            button {
                style: seg(!showing_inspect),
                onclick: move |_| inspect.set(false),
                { fl!("style-section-edit") }
            }
            button {
                style: seg(showing_inspect),
                onclick: move |_| inspect.set(true),
                { fl!("style-section-inspect") }
            }
        }
    }
}
