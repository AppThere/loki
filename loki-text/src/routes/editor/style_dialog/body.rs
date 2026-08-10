// SPDX-License-Identifier: Apache-2.0

//! Tab dispatch, the header breadcrumb, and the preview rail.

use std::rc::Rc;

use appthere_ui::DialogPosture;
use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::style::{StyleCatalog, StyleId};

use super::fields::{DraftSignal, OpenSignal};
use super::tabs::ParaTab;
use super::{
    preview, tab_alignment, tab_borders, tab_flow, tab_font, tab_general, tab_indents, tab_stops,
};

/// Maximum ancestors named in the header breadcrumb before it elides.
///
/// A chain deeper than this is rare and the header is not where it should be
/// read — the General tab draws the whole chain with the local/inherited counts.
const BREADCRUMB_MAX: usize = 4;

/// The style's ancestry, root first: `Default Paragraph Style → Body → Body indent`.
///
/// Walks the same `parent` links the resolver does, with the same cycle guard: a
/// self-referential chain in an imported document must not spin the header.
#[must_use]
pub(super) fn ancestry(catalog: &StyleCatalog, id: &StyleId) -> Vec<(StyleId, String)> {
    let mut chain: Vec<(StyleId, String)> = Vec::new();
    let mut seen: Vec<StyleId> = Vec::new();
    let mut cursor = Some(id.clone());
    while let Some(current) = cursor {
        if seen.contains(&current) {
            break;
        }
        seen.push(current.clone());
        let Some(style) = catalog.paragraph_styles.get(&current) else {
            break;
        };
        let display = style
            .display_name
            .clone()
            .unwrap_or_else(|| current.as_str().to_string());
        chain.push((current, display));
        cursor = style.parent.clone();
    }
    chain.reverse();
    chain
}

/// The header's breadcrumb line.
pub(super) fn breadcrumb(catalog: &StyleCatalog, id: &StyleId) -> Element {
    let chain = ancestry(catalog, id);
    let elided = chain.len() > BREADCRUMB_MAX;
    let shown: Vec<(StyleId, String)> = if elided {
        chain[chain.len() - BREADCRUMB_MAX..].to_vec()
    } else {
        chain
    };
    let last = shown.len().saturating_sub(1);

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: row; align-items: center; \
                 flex-wrap: wrap; gap: {gap}px; font-size: {fs}px; color: {fg};",
                gap = tokens::SPACE_1,
                fs = tokens::FONT_SIZE_META,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
            ),
            if elided {
                span { "\u{2026}" }
                span { "\u{2192}" }
            }
            // One keyed element per crumb, separator included — a key is only
            // honoured on the first node of a block.
            for (i, (sid, name)) in shown.iter().enumerate() {
                span {
                    key: "{sid.as_str()}",
                    style: format!(
                        "display: flex; flex-direction: row; align-items: center; gap: {gap}px;",
                        gap = tokens::SPACE_1,
                    ),
                    span {
                        style: if i == last {
                            format!("color: {};", tokens::COLOR_TEXT_ON_CHROME)
                        } else {
                            String::new()
                        },
                        {name.clone()}
                    }
                    if i != last {
                        span { "\u{2192}" }
                    }
                }
            }
        }
    }
}

/// Renders the active tab's body, with the preview rail docked beside it at
/// Expanded (design note 05).
#[allow(clippy::too_many_arguments)]
pub(super) fn tab_body(
    tab: ParaTab,
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
    font_families: Rc<Vec<String>>,
) -> Element {
    let form = match tab {
        ParaTab::General => tab_general::body(catalog, id, draft, posture),
        ParaTab::Font => tab_font::body(catalog, id, draft, open_style, posture, font_families),
        ParaTab::Indents => tab_indents::body(catalog, id, draft, open_style, posture),
        ParaTab::Alignment => tab_alignment::body(catalog, id, draft, open_style, posture),
        ParaTab::TextFlow => tab_flow::body(catalog, id, draft, open_style, posture),
        ParaTab::Borders => tab_borders::body(catalog, id, draft, open_style, posture),
        ParaTab::TabStops => tab_stops::body(catalog, id, draft, open_style, posture),
    };
    let show_preview = tab.has_preview();

    rsx! {
        div {
            style: "flex: 1; min-width: 0; display: flex; flex-direction: row;",
            {form}
            if show_preview && posture.preview_docked {
                div {
                    style: format!(
                        "width: {w}px; flex-shrink: 0; border-left: 1px solid {border}; \
                         display: flex; flex-direction: column;",
                        w = tokens::DIALOG_PREVIEW_RAIL_PX,
                        border = tokens::COLOR_BORDER_CHROME,
                    ),
                    { preview::rail(tab, catalog, id, draft) }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "body_tests.rs"]
mod tests;
