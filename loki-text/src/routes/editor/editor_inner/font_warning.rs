// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Font substitution warning banner for the document editor.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use crate::editing::state::DocumentState;

/// Computed font substitution data for the warning banner.
pub(super) struct FontData {
    /// Raw substitution map (empty = no warnings).
    pub substitutions: HashMap<String, Option<String>>,
    /// Human-readable summary of substituted and missing fonts.
    pub warning_details: String,
    /// Sorted, deduplicated download links for well-known missing fonts.
    pub download_links: Vec<(&'static str, &'static str)>,
}

/// Reads `doc_state` and builds all font-substitution data needed by the
/// warning banner.
pub(super) fn build_font_data(doc_state: &Arc<Mutex<DocumentState>>) -> FontData {
    let substitutions = if let Ok(state) = doc_state.lock() {
        if let Ok(fr) = state.shared_font_resources.lock() {
            fr.substitutions.clone()
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

    let mut substituted_items = Vec::new();
    let mut missing_items = Vec::new();
    let mut download_links: Vec<(&'static str, &'static str)> = Vec::new();

    for (requested, sub) in &substitutions {
        if let Some(sub_name) = sub {
            substituted_items.push(format!("{} \u{2192} {}", requested, sub_name));
        } else {
            missing_items.push(requested.clone());
        }
        if let Some(link) = known_font_download_link(requested) {
            download_links.push(link);
        }
    }

    download_links.sort_by_key(|(lbl, _)| *lbl);
    download_links.dedup_by_key(|(lbl, _)| *lbl);

    let sub_text = if !substituted_items.is_empty() {
        format!("Substituted: {}. ", substituted_items.join(", "))
    } else {
        String::new()
    };
    let miss_text = if !missing_items.is_empty() {
        format!("Missing: {}. ", missing_items.join(", "))
    } else {
        String::new()
    };

    FontData {
        substitutions,
        warning_details: format!("{}{}", sub_text, miss_text),
        download_links,
    }
}

/// Returns the download URL for a well-known font, if any.
pub(super) fn known_font_download_link(name: &str) -> Option<(&'static str, &'static str)> {
    match name.to_lowercase().as_str() {
        "aptos" => Some((
            "Aptos",
            "https://www.microsoft.com/en-us/download/details.aspx?id=106037",
        )),
        "calibri" => Some((
            "Calibri",
            "https://learn.microsoft.com/en-us/typography/font-list/calibri",
        )),
        "cambria" => Some((
            "Cambria",
            "https://learn.microsoft.com/en-us/typography/font-list/cambria",
        )),
        "arial" => Some((
            "Arial",
            "https://learn.microsoft.com/en-us/typography/font-list/arial",
        )),
        "courier new" => Some((
            "Courier New",
            "https://learn.microsoft.com/en-us/typography/font-list/courier-new",
        )),
        "times new roman" => Some((
            "Times New Roman",
            "https://learn.microsoft.com/en-us/typography/font-list/times-new-roman",
        )),
        _ => None,
    }
}

/// Renders the font substitution warning banner.
///
/// Shown when fonts in the document have been substituted or are missing.
/// Touch target: the dismiss button is ≥44×44 logical px (WCAG 2.5.8).
pub(super) fn font_warning_banner(
    font_warning_details: String,
    download_links: Vec<(&'static str, &'static str)>,
    mut dismiss_font_warning: Signal<bool>,
) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; justify-content: space-between; \
                 padding: {p}px {p2}px; background: {bg}; border-top: 1px solid {border}; \
                 border-bottom: 1px solid {border}; font-family: {ff}; font-size: {size}px; \
                 color: {fg}; flex-shrink: 0;",
                p      = tokens::SPACE_2,
                p2     = tokens::SPACE_4,
                bg     = tokens::COLOR_SURFACE_2,
                border = tokens::COLOR_CONTEXTUAL_TAB,
                ff     = tokens::FONT_FAMILY_UI,
                size   = tokens::FONT_SIZE_BODY - 1.0,
                fg     = tokens::COLOR_TEXT_ON_CHROME,
            ),
            div {
                style: "display: flex; flex-direction: column; gap: 4px; flex: 1;",
                div {
                    style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                    span {
                        style: format!("color: {}; font-weight: bold;", tokens::COLOR_CONTEXTUAL_TAB),
                        "⚠️ {fl!(\"editor-font-substitution-title\")}:"
                    }
                    span { {fl!("editor-font-substitution-message")} }
                }
                span {
                    style: format!("font-size: {size}px; color: {fg_sec};",
                        size   = tokens::FONT_SIZE_LABEL,
                        fg_sec = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    "{font_warning_details}"
                }
            }
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 16px; margin-left: 16px;",
                if !download_links.is_empty() {
                    div {
                        style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                        span {
                            style: format!("font-size: {size}px; color: {fg_sec};",
                                size   = tokens::FONT_SIZE_LABEL,
                                fg_sec = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            {fl!("editor-font-substitution-download")}
                        }
                        {
                            download_links.iter().map(|(label, url)| rsx! {
                                a {
                                    key: "{label}",
                                    style: format!(
                                        "color: {accent}; text-decoration: underline; font-size: {size}px; cursor: pointer;",
                                        accent = tokens::COLOR_TAB_ACTIVE_INDICATOR,
                                        size   = tokens::FONT_SIZE_LABEL,
                                    ),
                                    href: "{url}",
                                    target: "_blank",
                                    "{label}"
                                }
                            })
                        }
                    }
                }
                button {
                    style: format!(
                        "padding: {p}px {p2}px; background: {bg}; border: 1px solid {border}; \
                         border-radius: 4px; color: {fg}; font-size: {size}px; cursor: pointer; \
                         margin-left: 8px;",
                        p      = tokens::SPACE_1,
                        p2     = tokens::SPACE_2,
                        bg     = tokens::COLOR_SURFACE_3,
                        border = tokens::COLOR_BORDER_CHROME,
                        fg     = tokens::COLOR_TEXT_ON_CHROME,
                        size   = tokens::FONT_SIZE_LABEL,
                    ),
                    onclick: move |_| {
                        dismiss_font_warning.set(true);
                    },
                    {fl!("editor-font-dismiss")}
                }
            }
        }
    }
}
