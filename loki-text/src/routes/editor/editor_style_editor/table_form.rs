// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The editable table-style form (Spec 05 M6 table family, 4a.3): name,
//! based-on, table alignment, and the banding sizes. Apply cycle-guards the
//! based-on (`table_reparent_cycles`), commits through Loro (undoable), and
//! relays out — mirroring the character form. Fields the form does not edit
//! (width, background, the conditional/banding map) are preserved by
//! `draft_apply_to_table_style`.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::style::StyleId;
use loki_doc_model::style::table_style::TableAlignment;
use loki_i18n::fl;

use super::super::editor_keydown_ctrl::post_mutation_sync;
use super::super::editor_style_catalog::{
    catalog_snapshot, commit_table_style_to_loro, get_catalog_table_style,
};
use super::StyleEditorSync;
use super::draft_table::{TableStyleDraft, draft_apply_to_table_style};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Renders the table-style edit form for the active table draft.
pub(super) fn table_style_form(
    doc_state: Arc<Mutex<DocumentState>>,
    editing_table_draft: Signal<Option<TableStyleDraft>>,
    draft: TableStyleDraft,
    sync: StyleEditorSync,
) -> Element {
    rsx! {
        div {
            style: format!(
                "flex: 1; display: flex; flex-direction: column; gap: {g}px; \
                 padding: {p}px; overflow-y: auto;",
                g = tokens::SPACE_2,
                p = tokens::SPACE_3,
            ),

            div {
                style: format!(
                    "font-family: {ff}; font-size: {fs}px; color: {fg};",
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_XS,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-table-form-heading") }
            }

            { text_row(fl!("editor-style-name-label"), draft.name.clone(), editing_table_draft, |d, v| d.name = v) }
            { text_row(fl!("editor-style-based-on-label"), draft.parent.clone(), editing_table_draft, |d, v| d.parent = v) }
            { alignment_row(editing_table_draft, draft.alignment) }
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 16px; flex-wrap: wrap;",
                { text_row(fl!("style-table-row-band-label"), draft.row_band_str.clone(), editing_table_draft, |d, v| d.row_band_str = v) }
                { text_row(fl!("style-table-col-band-label"), draft.col_band_str.clone(), editing_table_draft, |d, v| d.col_band_str = v) }
            }

            { apply_button(doc_state, editing_table_draft, sync) }
        }
    }
}

/// A labelled single-line text input bound to one draft field.
fn text_row(
    label: String,
    value: String,
    mut draft: Signal<Option<TableStyleDraft>>,
    set: impl Fn(&mut TableStyleDraft, String) + 'static,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
            span {
                style: format!(
                    "font-family: {ff}; font-size: {fs}px; color: {fg}; min-width: 72px;",
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                "{label}"
            }
            input {
                style: format!(
                    "flex: 1; min-height: {h}px; padding: 0 {p}px; border-radius: {r}px; \
                     border: 1px solid {border}; background: {bg}; color: {fg}; \
                     font-family: {ff}; font-size: {fs}px;",
                    h = tokens::TOUCH_MIN,
                    p = tokens::SPACE_2,
                    r = tokens::RADIUS_SM,
                    border = tokens::COLOR_BORDER_CHROME,
                    bg = tokens::COLOR_SURFACE_2,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_BODY,
                ),
                value: "{value}",
                oninput: move |e| {
                    if let Some(d) = draft.write().as_mut() {
                        set(d, e.value());
                    }
                },
            }
        }
    }
}

/// Alignment segmented control: unset / left / centre / right.
///
/// # Touch target
///
/// Each segment is at least `TOUCH_MIN` (44 px) tall via `min-height`.
fn alignment_row(
    mut draft: Signal<Option<TableStyleDraft>>,
    current: Option<TableAlignment>,
) -> Element {
    let seg = |value: Option<TableAlignment>, label: String| {
        let active = current == value;
        rsx! {
            button {
                style: format!(
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
                    bg = if active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
                onclick: move |_| {
                    if let Some(d) = draft.write().as_mut() {
                        d.alignment = value;
                    }
                },
                "{label}"
            }
        }
    };
    rsx! {
        div {
            style: "display: flex; flex-direction: row; gap: 4px;",
            { seg(None, fl!("style-table-align-unset")) }
            { seg(Some(TableAlignment::Left), fl!("style-table-align-left")) }
            { seg(Some(TableAlignment::Center), fl!("style-table-align-center")) }
            { seg(Some(TableAlignment::Right), fl!("style-table-align-right")) }
        }
    }
}

/// The Apply button: cycle-guards the based-on, commits the table style
/// (preserving its banding map), and relays out.
fn apply_button(
    doc_state: Arc<Mutex<DocumentState>>,
    editing_table_draft: Signal<Option<TableStyleDraft>>,
    sync: StyleEditorSync,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; gap: 8px; margin-top: auto;",
            button {
                style: format!(
                    "padding: {p}px {p2}px; border-radius: {r}px; border: 1px solid {border}; \
                     cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                     background: {bg}; color: {fg};",
                    p = tokens::SPACE_1,
                    p2 = tokens::SPACE_3,
                    r = tokens::RADIUS_SM,
                    border = tokens::COLOR_TAB_ACTIVE_INDICATOR,
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_BODY,
                    bg = tokens::COLOR_SURFACE_3,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
                onclick: move |_| {
                    let Some(draft_val) = editing_table_draft.read().clone() else {
                        return;
                    };
                    // Reject a based-on that would form a cycle (Spec 05 §7).
                    if !draft_val.parent.is_empty() {
                        let child = StyleId::new(&draft_val.id);
                        let new_parent = StyleId::new(&draft_val.parent);
                        let cycles = catalog_snapshot(&doc_state)
                            .is_some_and(|cat| cat.table_reparent_cycles(&child, &new_parent));
                        if cycles {
                            let mut save_message = sync.save_message;
                            save_message.set(Some(fl!("style-reparent-cycle")));
                            return;
                        }
                    }
                    let base = get_catalog_table_style(&doc_state, &draft_val.id);
                    let style = draft_apply_to_table_style(&draft_val, base);
                    let applied = {
                        let guard = sync.loro_doc.read();
                        if let Some(ldoc) = guard.as_ref() {
                            commit_table_style_to_loro(ldoc, &doc_state, style);
                            apply_mutation_and_relayout(&doc_state, ldoc);
                            true
                        } else {
                            false
                        }
                    };
                    if applied {
                        post_mutation_sync(
                            &doc_state,
                            sync.loro_doc,
                            sync.cursor_state,
                            sync.undo_manager,
                            sync.can_undo,
                            sync.can_redo,
                        );
                    }
                },
                { fl!("editor-style-apply") }
            }
        }
    }
}
