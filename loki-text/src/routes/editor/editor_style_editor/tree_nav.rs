// SPDX-License-Identifier: Apache-2.0

//! Compact breadcrumb + drill-down for the paragraph inheritance tree (Spec 05
//! §7/§11, M7).
//!
//! At Compact the full indented tree is impractical, so it degrades to a
//! **breadcrumb** (the root→selected path, each hop clickable to jump up) plus a
//! **drill-down** into the selected style's direct substyles (clickable to
//! descend). Both use the model's cycle-guarded `para_breadcrumb` /
//! `para_children`; navigation loads the target style's draft, exactly as the
//! Expanded indented tree does.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::style::StyleId;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_i18n::fl;

use super::super::editor_state::StyleDraft;
use super::super::editor_style_catalog::{catalog_snapshot, get_catalog_style};
use super::posture::StylePanelPosture;
use super::style_to_draft;
use crate::editing::state::DocumentState;

/// A style's display name, falling back to its id.
fn display_of(catalog: &StyleCatalog, id: &StyleId) -> String {
    catalog
        .paragraph_styles
        .get(id)
        .and_then(|s| s.display_name.clone())
        .unwrap_or_else(|| id.as_str().to_string())
}

/// A navigation button that loads style `id`'s draft when pressed. `active`
/// highlights the current style; `sep` renders a leading "›" breadcrumb divider.
fn nav_button(
    doc_state: &Arc<Mutex<DocumentState>>,
    id: String,
    label: String,
    active: bool,
    sep: bool,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    posture: StylePanelPosture,
) -> Element {
    let ds = Arc::clone(doc_state);
    rsx! {
        if sep {
            span {
                style: format!(
                    "color: {fg}; font-size: {fs}px; align-self: center;",
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    fs = tokens::FONT_SIZE_LABEL,
                ),
                "\u{203A}"
            }
        }
        button {
            style: format!(
                "text-align: left; padding: {p}px {p2}px; border-radius: 3px; {touch} \
                 border: 1px solid {border}; cursor: pointer; font-family: {ff}; \
                 font-size: {fs}px; background: {bg}; color: {fg};",
                p = tokens::SPACE_1,
                p2 = tokens::SPACE_2,
                touch = posture.touch_min_css(),
                border = if active { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                ff = tokens::FONT_FAMILY_UI,
                fs = tokens::FONT_SIZE_LABEL,
                bg = if active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            onclick: move |_| {
                if let Some(s) = get_catalog_style(&ds, &id) {
                    editing_style_draft.set(Some(style_to_draft(&s)));
                }
            },
            "{label}"
        }
    }
}

/// Renders the Compact breadcrumb (root→selected) and the selected style's
/// direct substyles. Returns an empty element if the catalog is unavailable.
pub(super) fn compact_tree_nav(
    doc_state: Arc<Mutex<DocumentState>>,
    active_id: String,
    editing_style_draft: Signal<Option<StyleDraft>>,
    posture: StylePanelPosture,
) -> Element {
    let Some(catalog) = catalog_snapshot(&doc_state) else {
        return rsx! {};
    };
    let sel = StyleId::new(&active_id);
    let path = catalog.para_breadcrumb(&sel);
    let children = catalog.para_children(&sel);

    rsx! {
        // Breadcrumb: root → … → selected, each hop clickable.
        div {
            style: "display: flex; flex-direction: row; flex-wrap: wrap; align-items: center; gap: 2px;",
            for (i, id) in path.iter().enumerate() {
                {
                    let label = display_of(&catalog, id);
                    let is_last = i + 1 == path.len();
                    nav_button(&doc_state, id.as_str().to_string(), label, is_last, i > 0, editing_style_draft, posture)
                }
            }
        }

        // Drill-down: the selected style's direct substyles (if any).
        if !children.is_empty() {
            div {
                style: format!(
                    "font-size: {fs}px; color: {fg}; margin-top: {mt}px; margin-bottom: 2px;",
                    fs = tokens::FONT_SIZE_XS,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    mt = tokens::SPACE_2,
                ),
                { fl!("style-tree-substyles-heading") }
            }
            for id in children.iter() {
                {
                    let label = display_of(&catalog, id);
                    nav_button(&doc_state, id.as_str().to_string(), label, false, false, editing_style_draft, posture)
                }
            }
        }
    }
}
