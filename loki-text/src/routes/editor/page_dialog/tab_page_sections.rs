// SPDX-License-Identifier: Apache-2.0

//! The Page tab's **Sections** block (§3b): one row per document section
//! showing the page style it currently uses, with an "apply this style"
//! action per section — the assignment side of named page styles, which had
//! model + mutation (`set_section_page_style`) but no UI.
//!
//! # Touch target
//!
//! Each apply button is at least the dialog posture's touch minimum tall
//! (44 px at Compact, WCAG 2.5.8).

use std::sync::{Arc, Mutex};

use appthere_ui::{AtField, DialogPosture, tokens};
use dioxus::prelude::*;
use loki_doc_model::loro_mutation::set_section_page_style;
use loki_i18n::fl;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::super::editor_style_editor::StyleEditorSync;
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// One section's row: index, and the display name of its current style.
pub(super) struct SectionRow {
    pub index: usize,
    pub style: Option<String>,
}

/// Snapshots the document's sections for the block. Empty (hiding the block)
/// only when there is no document.
pub(super) fn section_rows(doc_state: &Arc<Mutex<DocumentState>>) -> Vec<SectionRow> {
    let Ok(state) = doc_state.lock() else {
        return Vec::new();
    };
    let Some(doc) = state.document.as_ref() else {
        return Vec::new();
    };
    doc.sections
        .iter()
        .enumerate()
        .map(|(index, s)| SectionRow {
            index,
            style: s.page_style.as_ref().map(|p| p.as_str().to_string()),
        })
        .collect()
}

/// Renders the Sections block: hidden for the common single-section document
/// already using the edited style (nothing to reassign), shown otherwise.
pub(super) fn sections_block(
    doc_state: &Arc<Mutex<DocumentState>>,
    edited_style: String,
    sync: StyleEditorSync,
    posture: DialogPosture,
) -> Element {
    let rows = section_rows(doc_state);
    let all_this_style = rows
        .iter()
        .all(|r| r.style.as_deref() == Some(edited_style.as_str()));
    if rows.len() <= 1 && all_this_style {
        return rsx! {};
    }
    let ds = Arc::clone(doc_state);

    rsx! {
        AtField {
            label: fl!("page-dialog-sections"),
            control: rsx! {
                div {
                    style: format!(
                        "display: flex; flex-direction: column; gap: {g}px;",
                        g = tokens::SPACE_1,
                    ),
                    for row in rows.into_iter() {
                        {section_row(&ds, &edited_style, sync, posture, row)}
                    }
                }
            },
        }
    }
}

fn section_row(
    doc_state: &Arc<Mutex<DocumentState>>,
    edited_style: &str,
    sync: StyleEditorSync,
    posture: DialogPosture,
    row: SectionRow,
) -> Element {
    let uses_this = row.style.as_deref() == Some(edited_style);
    let style_label = row
        .style
        .clone()
        .unwrap_or_else(|| fl!("page-dialog-section-unstyled"));
    let ds = Arc::clone(doc_state);
    let target = edited_style.to_string();
    let index = row.index;

    rsx! {
        div {
            key: "section-{index}",
            style: format!(
                "display: flex; flex-direction: row; align-items: center; \
                 justify-content: space-between; gap: {g}px;",
                g = tokens::SPACE_2,
            ),
            span {
                style: format!(
                    "font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
                { fl!("page-dialog-section-row", n = (index + 1) as i64, style = style_label) }
            }
            button {
                style: format!(
                    "padding: {p}px {p2}px; min-height: {t}px; border-radius: 3px; \
                     cursor: pointer; font-size: {fs}px; border: 1px solid {border}; \
                     background: {bg}; color: {fg};",
                    p = tokens::SPACE_1,
                    p2 = tokens::SPACE_2,
                    t = posture.min_touch_px,
                    fs = tokens::FONT_SIZE_LABEL,
                    border = tokens::COLOR_BORDER_CHROME,
                    bg = tokens::COLOR_SURFACE_2,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
                disabled: uses_this,
                onclick: move |_| {
                    // The commit pipeline the dialog's Apply uses: guard,
                    // mutate, relayout, release, sync — in that order.
                    let guard = sync.loro_doc.read();
                    let Some(ldoc) = guard.as_ref() else { return };
                    if set_section_page_style(ldoc, index, &target).is_err() {
                        return;
                    }
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
                },
                { if uses_this { fl!("page-dialog-section-current") } else { fl!("page-dialog-section-apply") } }
            }
        }
    }
}
