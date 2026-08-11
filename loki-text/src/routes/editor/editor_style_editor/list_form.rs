// SPDX-License-Identifier: Apache-2.0

//! Right-column **list-style level** edit form (§10 tier 5): the per-level
//! form the read-only browser lacked. One level is edited at a time — the
//! inspector's level rows seed the draft (see `family_inspector`) — and
//! Apply commits through [`set_list_style_level`], so every referencing
//! paragraph's markers re-derive on the following relayout.
//!
//! # Touch target
//!
//! The kind toggle, scheme buttons, and Apply are `TOUCH_MIN`-tall (44 px,
//! WCAG 2.5.8) via the posture-supplied minimum on every `button`.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::loro_mutation::set_list_style_level;
use loki_doc_model::style::ListId;
use loki_i18n::fl;

use super::StyleEditorSync;
use super::list_form_draft::{DraftKind, ListLevelDraft, SCHEME_CHOICES};
use super::page_commit::commit;
use crate::editing::state::DocumentState;

/// A labelled single-line text field bound to one draft string field.
fn draft_field(
    label: String,
    value: String,
    width: &'static str,
    mut draft: Signal<Option<ListLevelDraft>>,
    write: fn(&mut ListLevelDraft, String),
) -> Element {
    rsx! {
        label {
            style: format!(
                "display: flex; flex-direction: column; gap: 2px; font-size: {fs}px; color: {fg};",
                fs = tokens::FONT_SIZE_XS,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
            ),
            { label }
            input {
                style: format!(
                    "{width}; padding: {p}px; border-radius: 3px; border: 1px solid {border}; \
                     background: {bg}; color: {fg}; font-size: {fs}px;",
                    p = tokens::SPACE_1,
                    border = tokens::COLOR_BORDER_CHROME,
                    bg = tokens::COLOR_SURFACE_2,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                    fs = tokens::FONT_SIZE_LABEL,
                ),
                value,
                oninput: move |evt| {
                    if let Some(d) = draft.write().as_mut() {
                        write(d, evt.value());
                    }
                },
            }
        }
    }
}

/// One small toggle/choice button.
fn choice_button(label: String, active: bool, onclick: EventHandler<()>) -> Element {
    rsx! {
        button {
            style: format!(
                "padding: {p}px {p2}px; min-height: {t}px; border-radius: 3px; cursor: pointer; \
                 font-size: {fs}px; border: 1px solid {border}; background: {bg}; color: {fg};",
                p = tokens::SPACE_1,
                p2 = tokens::SPACE_2,
                t = tokens::TOUCH_MIN,
                fs = tokens::FONT_SIZE_LABEL,
                border = if active {
                    tokens::COLOR_TAB_ACTIVE_INDICATOR
                } else {
                    tokens::COLOR_BORDER_CHROME
                },
                bg = if active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            onclick: move |_| onclick.call(()),
            { label }
        }
    }
}

/// Builds the level-row click handler for the family inspector: snapshots
/// catalog level `n` of the selected list style into the draft signal, which
/// mounts the form. `None` selection yields a handler that does nothing.
pub(super) fn seed_level_handler(
    doc_state: &Arc<Mutex<DocumentState>>,
    list_selected: Option<String>,
    mut editing_list_level: Signal<Option<ListLevelDraft>>,
) -> EventHandler<u8> {
    let ds = Arc::clone(doc_state);
    EventHandler::new(move |level: u8| {
        let Some(id) = list_selected.as_deref() else {
            return;
        };
        let snapshot = {
            let Ok(state) = ds.lock() else { return };
            let Some(doc) = state.document.as_ref() else {
                return;
            };
            let Some(style) = doc.styles.list_styles.get(&ListId::new(id)) else {
                return;
            };
            style
                .levels
                .get(usize::from(level))
                .map(|lvl| ListLevelDraft::from_level(id, lvl))
        };
        if let Some(draft) = snapshot {
            editing_list_level.set(Some(draft));
        }
    })
}

/// Renders the level edit form for the active list-level draft.
pub(super) fn list_level_form(
    doc_state: Arc<Mutex<DocumentState>>,
    mut editing_list_level: Signal<Option<ListLevelDraft>>,
    draft: ListLevelDraft,
    sync: StyleEditorSync,
) -> Element {
    let is_numbered = draft.kind == DraftKind::Numbered;
    let ds_apply = Arc::clone(&doc_state);

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {g}px; padding: {p}px; \
                 width: 240px; min-width: 240px; overflow-y: auto; \
                 border-left: 1px solid {border};",
                g = tokens::SPACE_2,
                p = tokens::SPACE_3,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            div {
                style: format!(
                    "font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_XS,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-list-form-heading", n = i64::from(draft.level)) }
            }

            // ── Kind toggle ───────────────────────────────────────────────────
            div {
                style: format!("display: flex; flex-direction: row; gap: {}px;", tokens::SPACE_1),
                { choice_button(fl!("style-list-kind-bullet"), !is_numbered, EventHandler::new({
                    move |()| {
                        if let Some(d) = editing_list_level.write().as_mut() {
                            d.kind = DraftKind::Bullet;
                        }
                    }
                })) }
                { choice_button(fl!("style-list-kind-numbered"), is_numbered, EventHandler::new({
                    move |()| {
                        if let Some(d) = editing_list_level.write().as_mut() {
                            d.kind = DraftKind::Numbered;
                        }
                    }
                })) }
            }

            // ── Kind-specific fields ─────────────────────────────────────────
            if is_numbered {
                div {
                    style: format!("display: flex; flex-direction: row; flex-wrap: wrap; gap: {}px;", tokens::SPACE_1),
                    for (scheme, label) in SCHEME_CHOICES.iter() {
                        { choice_button(
                            (*label).to_string(),
                            draft.scheme == *scheme,
                            EventHandler::new({
                                let scheme = *scheme;
                                move |()| {
                                    if let Some(d) = editing_list_level.write().as_mut() {
                                        d.scheme = scheme;
                                    }
                                }
                            }),
                        ) }
                    }
                }
                div {
                    style: "display: flex; flex-direction: row; gap: 8px;",
                    { draft_field(fl!("style-list-format-label"), draft.format.clone(), "width: 72px", editing_list_level, |d, v| d.format = v) }
                    { draft_field(fl!("style-list-start-label"), draft.start.clone(), "width: 48px", editing_list_level, |d, v| d.start = v) }
                }
            } else {
                { draft_field(fl!("style-list-bullet-label"), draft.bullet_char.clone(), "width: 48px", editing_list_level, |d, v| d.bullet_char = v) }
            }

            // ── Geometry ─────────────────────────────────────────────────────
            div {
                style: "display: flex; flex-direction: row; gap: 8px;",
                { draft_field(fl!("style-list-indent-label"), draft.indent.clone(), "width: 56px", editing_list_level, |d, v| d.indent = v) }
                { draft_field(fl!("style-list-hanging-label"), draft.hanging.clone(), "width: 56px", editing_list_level, |d, v| d.hanging = v) }
            }

            // ── Apply ────────────────────────────────────────────────────────
            button {
                style: format!(
                    "padding: {p}px {p2}px; min-height: {t}px; border-radius: {r}px; \
                     border: 1px solid {border}; cursor: pointer; font-size: {fs}px; \
                     background: {bg}; color: {fg}; margin-top: auto;",
                    p = tokens::SPACE_1,
                    p2 = tokens::SPACE_3,
                    t = tokens::TOUCH_MIN,
                    r = tokens::RADIUS_SM,
                    border = tokens::COLOR_TAB_ACTIVE_INDICATOR,
                    fs = tokens::FONT_SIZE_BODY,
                    bg = tokens::COLOR_SURFACE_3,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
                onclick: move |_| {
                    let Some(d) = editing_list_level.read().clone() else {
                        return;
                    };
                    // The existing level supplies the unexposed fields; a
                    // draft whose numerics do not parse declines here.
                    let existing = {
                        let Ok(state) = ds_apply.lock() else { return };
                        let Some(doc) = state.document.as_ref() else {
                            return;
                        };
                        let Some(style) =
                            doc.styles.list_styles.get(&ListId::new(&d.style_id))
                        else {
                            return;
                        };
                        let Some(lvl) = style.levels.get(usize::from(d.level)) else {
                            return;
                        };
                        lvl.clone()
                    };
                    let Some(new_level) = d.to_level(&existing) else {
                        return;
                    };
                    commit(&ds_apply, sync, |ldoc| {
                        set_list_style_level(ldoc, &d.style_id, d.level, new_level)
                    });
                },
                { fl!("ribbon-style-apply-changes") }
            }
        }
    }
}
