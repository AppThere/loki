// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The table-styles list for the style panel's left column (Spec 05 M6 table
//! family, 4a.3). Selecting a table style seeds `editing_table_draft` from
//! the catalog entry, which the panel renders as the editable table form.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_style_catalog::{catalog_snapshot, get_catalog_table_style};
use super::draft_table::{TableStyleDraft, table_style_to_draft};
use super::posture::StylePanelPosture;
use crate::editing::state::DocumentState;

/// Renders the "Table styles" heading and one button per catalog table style
/// (empty when the document has none). Clicking a style selects it and seeds
/// the editable draft. `posture` supplies the Compact touch minimum (§11).
pub(super) fn table_list_section(
    doc_state: Arc<Mutex<DocumentState>>,
    table_list: Vec<(String, String)>,
    table_selected: Option<String>,
    mut editing_table_style: Signal<Option<String>>,
    mut editing_table_draft: Signal<Option<TableStyleDraft>>,
    posture: StylePanelPosture,
) -> Element {
    if table_list.is_empty() {
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
            { fl!("style-table-family-heading") }
        }
        for (id, display) in table_list.into_iter() {
            {
                let is_sel = table_selected.as_deref() == Some(id.as_str());
                let id_cap = id.clone();
                let ds_c = Arc::clone(&doc_state);
                rsx! {
                    button {
                        key: "table-{id}",
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
                            editing_table_style.set(Some(id_cap.clone()));
                            // Seed the editable draft from the selected style.
                            if let Some(s) = get_catalog_table_style(&ds_c, &id_cap) {
                                editing_table_draft.set(Some(table_style_to_draft(&s)));
                            }
                        },
                        "{display}"
                    }
                }
            }
        }
    }
}

/// The table-styles list for the browser (Spec 05 M6 table family, 4a.3):
/// `(id, display name)` per catalog table style, synthetics (`__…`) hidden.
pub(super) fn table_data(doc_state: &Arc<Mutex<DocumentState>>) -> Vec<(String, String)> {
    let Some(catalog) = catalog_snapshot(doc_state) else {
        return Vec::new();
    };
    let mut list: Vec<(String, String)> = catalog
        .table_styles
        .iter()
        .filter(|(id, _)| !id.as_str().starts_with("__"))
        .map(|(id, s)| {
            let display = s
                .display_name
                .clone()
                .unwrap_or_else(|| id.as_str().to_string());
            (id.as_str().to_string(), display)
        })
        .collect();
    list.sort_by(|a, b| a.1.cmp(&b.1));
    list
}
