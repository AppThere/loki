// SPDX-License-Identifier: Apache-2.0

//! The page-styles list for the style panel's left column (Spec 05 M6 page
//! family, ADR-0012 Decision 2). Mirrors `list_browser` — a non-inheriting
//! browse-and-inspect surface, kept in its own module to hold `mod.rs` under the
//! ceiling.
//!
//! Selecting a page style writes its id into `editing_page_style`, which the
//! panel reads to show that style's geometry rows read-only (§9). Page styles
//! are non-inheriting, so this is a plain list (a flat family, like lists).

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::create_page_style;
use loki_i18n::fl;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::StyleEditorSync;
use super::page_form::button_css;
use super::panel_data_page::{PageListEntry, next_page_style_name, page_edit_target};
use super::posture::StylePanelPosture;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// The "New page style" button: creates the next free `PageStyleN`, seeded from
/// the selected style's geometry (else the document default), and selects it so
/// the form opens on it — the user renames it there.
///
/// The new style is applied to no section yet, which is why the browser lists
/// unapplied styles at all: a style that appeared nowhere until it was in use
/// could not be reached to put it in use.
fn new_page_style_button(
    doc_state: &Arc<Mutex<DocumentState>>,
    selected: Option<String>,
    mut editing_page_style: Signal<Option<String>>,
    sync: StyleEditorSync,
) -> Element {
    let ds = Arc::clone(doc_state);
    rsx! {
        button {
            style: button_css(false),
            onclick: move |_| {
                let Some(name) = next_page_style_name(&ds) else { return };
                let seed = selected
                    .as_deref()
                    .and_then(|s| page_edit_target(&ds, s))
                    .unwrap_or_default();
                let guard = sync.loro_doc.read();
                let Some(ldoc) = guard.as_ref() else { return };
                if create_page_style(ldoc, &name, &seed).is_ok() {
                    apply_mutation_and_relayout(&ds, ldoc);
                    drop(guard);
                    post_mutation_sync(
                        &ds,
                        sync.loro_doc,
                        sync.cursor_state,
                        sync.undo_manager,
                        sync.can_undo,
                        sync.can_redo,
                    );
                    editing_page_style.set(Some(name));
                }
            },
            { fl!("style-page-new") }
        }
    }
}

/// Renders the "Page styles" heading, the New button, and one button per page
/// style. `page_selected` highlights the active id; `posture` supplies the
/// Compact touch minimum (§11). A style no section uses is marked, so "created
/// but not applied" is visible rather than indistinguishable from in-use.
pub(super) fn page_list_section(
    doc_state: &Arc<Mutex<DocumentState>>,
    page_list: Vec<PageListEntry>,
    page_selected: Option<String>,
    mut editing_page_style: Signal<Option<String>>,
    posture: StylePanelPosture,
    sync: StyleEditorSync,
) -> Element {
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
        { new_page_style_button(doc_state, page_selected.clone(), editing_page_style, sync) }
        for (id, display, applied) in page_list.into_iter() {
            {
                let is_sel = page_selected.as_deref() == Some(id.as_str());
                let id_cap = id.clone();
                rsx! {
                    button {
                        key: "page-{id}",
                        style: format!(
                            "text-align: left; padding: {p}px {p2}px; border-radius: 3px; {touch} \
                             border: 1px solid {border}; cursor: pointer; \
                             font-size: {fs}px; background: {bg}; color: {fg};",
                            p = tokens::SPACE_1,
                            p2 = tokens::SPACE_2,
                            touch = posture.touch_min_css(),
                            border = if is_sel {
                                tokens::COLOR_TAB_ACTIVE_INDICATOR
                            } else {
                                tokens::COLOR_BORDER_CHROME
                            },
                            fs = tokens::FONT_SIZE_LABEL,
                            bg = if is_sel { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        onclick: move |_| editing_page_style.set(Some(id_cap.clone())),
                        if applied { "{display}" } else { { fl!("style-page-unapplied", name = display.clone()) } }
                    }
                }
            }
        }
    }
}
