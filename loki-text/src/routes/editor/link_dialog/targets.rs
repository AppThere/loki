// SPDX-License-Identifier: Apache-2.0

//! The in-document target picker (design note 20).
//!
//! Targets are **picked from the live outline**, never typed: an anchor typed
//! by hand is a link that silently stops working the moment the heading it
//! names is renamed, and the document already knows every place it can reach.

use std::sync::{Arc, Mutex};

use appthere_ui::{DialogPosture, at_field_label_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::LinkDraft;
use super::target::{OutlineTarget, TargetKind, document_targets};
use crate::editing::state::DocumentState;

/// The in-document target list's height. Bounded so the list cannot push the
/// display-text fields off a Compact sheet.
const TARGET_LIST_MAX_PX: f32 = 220.0;

/// The in-document picker: a filter box over one list of headings and
/// bookmarks, with the kind as a right-aligned label (design note 20).
pub(super) fn target_picker(
    doc_state: &Arc<Mutex<DocumentState>>,
    mut draft: Signal<Option<LinkDraft>>,
    posture: DialogPosture,
    current: &LinkDraft,
) -> Element {
    let targets = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| document_targets(d)))
        .unwrap_or_default();
    let query = current.filter.to_lowercase();
    let matches: Vec<OutlineTarget> = targets
        .into_iter()
        .filter(|t| query.is_empty() || t.label.to_lowercase().contains(&query))
        .collect();
    let selected = current.document.clone();
    let filter = current.filter.clone();
    let row_height = posture.min_touch_px.max(tokens::TOUCH_MIN);

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px;",
                gap = tokens::SPACE_1,
            ),
            div { style: at_field_label_style(), { fl!("link-dialog-target") } }
            div {
                style: format!(
                    "border: 1px solid {border}; border-radius: {r}px; overflow: hidden; \
                     background: {bg};",
                    border = tokens::COLOR_BORDER_CHROME,
                    r = tokens::RADIUS_MD,
                    bg = tokens::COLOR_SURFACE_2,
                ),
                div {
                    style: format!(
                        "display: flex; flex-direction: row; align-items: center; \
                         gap: {gap}px; min-height: {h}px; padding: 0 {px}px; \
                         border-bottom: 1px solid {border};",
                        gap = tokens::SPACE_2,
                        h = row_height,
                        px = tokens::SPACE_3,
                        border = tokens::COLOR_BORDER_CHROME,
                    ),
                    span { style: format!("color: {};", tokens::COLOR_TEXT_ON_CHROME_SECONDARY), "\u{2315}" }
                    input {
                        r#type: "text",
                        value: "{filter}",
                        style: format!(
                            "flex: 1; min-width: 0; background: transparent; border: none; \
                             font-size: {fs}px; color: {fg};",
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        oninput: move |evt| {
                            let mut next = draft.read().clone();
                            if let Some(d) = next.as_mut() {
                                d.filter = evt.value();
                            }
                            draft.set(next);
                        },
                    }
                }

                if matches.is_empty() {
                    div {
                        style: format!(
                            "padding: {p}px; font-size: {fs}px; color: {fg};",
                            p = tokens::SPACE_4,
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        { fl!("link-dialog-no-targets") }
                    }
                }

                div {
                    style: format!("max-height: {TARGET_LIST_MAX_PX}px; overflow-y: auto;"),
                    for t in matches.iter() {
                        {
                            let href = t.href();
                            let is_current = selected == href;
                            let indent = f32::from(t.depth()) * tokens::SPACE_3;
                            rsx! {
                                button {
                                    key: "{t.anchor}",
                                    style: format!(
                                        "width: 100%; min-height: {h}px; box-sizing: border-box; \
                                         display: flex; flex-direction: row; align-items: center; \
                                         justify-content: space-between; gap: {gap}px; \
                                         padding: 0 {px}px 0 {pl}px; text-align: left; \
                                         cursor: pointer; background: {bg}; border: none; \
                                         border-bottom: 1px solid {border}; {edge} \
                                         font-size: {fs}px; color: {fg};",
                                        h = row_height,
                                        gap = tokens::SPACE_2,
                                        px = tokens::SPACE_3,
                                        pl = tokens::SPACE_3 + indent,
                                        bg = if is_current { tokens::COLOR_SURFACE_3 } else { "transparent" },
                                        border = tokens::COLOR_BORDER_CHROME,
                                        edge = if is_current {
                                            format!("border-left: 3px solid {};", tokens::COLOR_TAB_ACTIVE_INDICATOR)
                                        } else {
                                            String::new()
                                        },
                                        fs = tokens::FONT_SIZE_BODY,
                                        fg = tokens::COLOR_TEXT_ON_CHROME,
                                    ),
                                    onclick: move |evt| {
                                        evt.stop_propagation();
                                        let mut next = draft.read().clone();
                                        if let Some(d) = next.as_mut() {
                                            d.document = href.clone();
                                        }
                                        draft.set(next);
                                    },
                                    span { {t.label.clone()} }
                                    span {
                                        style: format!(
                                            "flex-shrink: 0; font-size: {fs}px; color: {fg};",
                                            fs = tokens::FONT_SIZE_XS,
                                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                        ),
                                        {
                                            match t.kind {
                                                TargetKind::Heading(level) => {
                                                    fl!("link-dialog-target-heading", level = i64::from(level))
                                                }
                                                TargetKind::Bookmark => fl!("link-dialog-target-bookmark"),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
