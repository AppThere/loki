// SPDX-License-Identifier: Apache-2.0

//! The font-family field: a filter box over one list, bundled faces first
//! (design notes 07, 08 and 10).
//!
//! # The list is in flow, not a popover
//!
//! It renders directly under the input rather than as an anchored overlay. The
//! anchor would sit in an *inline* formatting context, where `position:
//! absolute` is still unverified in Blitz (root `CLAUDE.md`), and a popover that
//! exceeds the viewport is the worst small-screen failure mode there is (design
//! note 10 makes the same call for the Compact picker). The list only appears
//! while the field is focused, so it costs nothing at rest.

use std::rc::Rc;

use appthere_ui::tokens;
use appthere_ui::{AtField, DialogPosture, at_control_style};
use dioxus::prelude::*;
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::fields::{DraftSignal, OpenSignal, provenance_line};
use super::font_list::{FontRow, font_sections};
use super::rows::resolve_row;

/// Height of the in-flow result list. Bounded so the list cannot push the rest
/// of the form off a Compact sheet.
const LIST_MAX_HEIGHT_PX: f32 = 220.0;

/// Renders the font-family field.
pub(super) fn field(
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
    font_families: Rc<Vec<String>>,
) -> Element {
    let mut query = use_signal(String::new);
    let mut open = use_signal(|| false);

    let selected = draft
        .read()
        .as_ref()
        .and_then(|d| d.style.char_props.font_name.clone());
    let row = resolve_row(
        catalog,
        id,
        |s| s.char_props.font_name.clone(),
        Clone::clone,
    );
    let sections = font_sections(&font_families, &query.read());
    let is_open = *open.read();
    let bundled = loki_fonts::is_bundled_family(selected.as_deref().unwrap_or_default());

    rsx! {
        AtField {
            label: fl!("style-dialog-font-family"),
            extra_style: super::fields::full_width(posture),
            control: rsx! {
                div {
                    style: "display: flex; flex-direction: column; gap: 0;",

                    // The trigger doubles as the filter box while open.
                    div {
                        style: at_control_style(posture.min_touch_px, "width: 100%;"),
                        if is_open {
                            input {
                                r#type: "text",
                                value: "{query}",
                                style: format!(
                                    "flex: 1; min-width: 0; background: transparent; \
                                     border: none; font-size: {fs}px; color: {fg};",
                                    fs = tokens::FONT_SIZE_MD,
                                    fg = tokens::COLOR_TEXT_ON_CHROME,
                                ),
                                oninput: move |evt| query.set(evt.value()),
                            }
                        } else {
                            button {
                                style: "flex: 1; min-width: 0; display: flex; \
                                        flex-direction: row; align-items: center; gap: 8px; \
                                        background: transparent; border: none; \
                                        cursor: pointer; text-align: left;",
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    open.set(true);
                                },
                                span {
                                    style: format!(
                                        "font-size: {fs}px; color: {fg};",
                                        fs = tokens::FONT_SIZE_MD,
                                        fg = tokens::COLOR_TEXT_ON_CHROME,
                                    ),
                                    {
                                        selected
                                            .clone()
                                            .unwrap_or_else(|| fl!("style-dialog-font-family-inherit"))
                                    }
                                }
                                if bundled {
                                    { badge() }
                                }
                            }
                        }
                        span {
                            style: format!(
                                "flex-shrink: 0; color: {};",
                                tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            if is_open { "\u{25B4}" } else { "\u{25BE}" }
                        }
                    }

                    if is_open {
                        div {
                            style: format!(
                                "max-height: {h}px; overflow-y: auto; margin-top: {mt}px; \
                                 background: {bg}; border: 1px solid {border}; \
                                 border-radius: {r}px;",
                                h = LIST_MAX_HEIGHT_PX,
                                mt = tokens::SPACE_1,
                                bg = tokens::COLOR_SURFACE_2,
                                border = tokens::COLOR_BORDER_CHROME,
                                r = tokens::RADIUS_MD,
                            ),

                            if sections.is_empty() {
                                div {
                                    style: format!(
                                        "padding: {p}px; font-size: {fs}px; color: {fg};",
                                        p = tokens::SPACE_3,
                                        fs = tokens::FONT_SIZE_BODY,
                                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                    ),
                                    { fl!("style-dialog-font-no-matches") }
                                }
                            }

                            if !sections.bundled.is_empty() {
                                { heading(fl!("style-dialog-font-bundled-heading")) }
                                for row in sections.bundled.iter() {
                                    { entry(row.clone(), selected.clone(), posture, draft, open, query) }
                                }
                            }
                            if !sections.device.is_empty() {
                                { heading(fl!(
                                    "style-dialog-font-device-heading",
                                    count = sections.device.len() as i64
                                )) }
                                for row in sections.device.iter() {
                                    { entry(row.clone(), selected.clone(), posture, draft, open, query) }
                                }
                            }
                        }
                    }
                }
            },
            footnote: row
                .as_ref()
                .map(|(source, value)| {
                    provenance_line(
                        source,
                        value.as_deref(),
                        posture,
                        open_style,
                        draft,
                        |d| d.style.char_props.font_name = None,
                    )
                })
                .unwrap_or_else(|| rsx! {}),
        }
    }
}

/// The `Bundled` badge — the mark that this face renders for every reader.
fn badge() -> Element {
    rsx! {
        span {
            style: format!(
                "flex-shrink: 0; padding: 1px {px}px; border-radius: {r}px; \
                 border: 1px solid {border}; font-size: {fs}px; color: {fg};",
                px = tokens::SPACE_1,
                r = tokens::RADIUS_SM,
                border = tokens::COLOR_BORDER_CHROME,
                fs = tokens::FONT_SIZE_XS,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
            ),
            { fl!("style-dialog-font-bundled-badge") }
        }
    }
}

/// A section heading inside the list.
fn heading(text: String) -> Element {
    rsx! {
        div {
            style: format!(
                "padding: {py}px {px}px; font-size: {fs}px; font-weight: {fw}; \
                 letter-spacing: 0.06em; text-transform: uppercase; \
                 color: {fg}; border-bottom: 1px solid {border};",
                py = tokens::SPACE_2,
                px = tokens::SPACE_3,
                fs = tokens::FONT_SIZE_XS,
                fw = tokens::FONT_WEIGHT_SEMIBOLD,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                border = tokens::COLOR_BORDER_CHROME,
            ),
            {text}
        }
    }
}

/// One family row. The current face carries the accent edge (design note 10).
fn entry(
    row: FontRow,
    selected: Option<String>,
    posture: DialogPosture,
    mut draft: DraftSignal,
    mut open: Signal<bool>,
    mut query: Signal<String>,
) -> Element {
    let is_current = selected
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case(&row.name));
    let name = row.name.clone();
    // Rows are touch-sized at every size class: this list is the only route to
    // a face, and it is scrolled with a finger on any device that has one.
    let height = posture.min_touch_px.max(tokens::TOUCH_MIN);

    rsx! {
        button {
            style: format!(
                "width: 100%; min-height: {h}px; box-sizing: border-box; \
                 display: flex; flex-direction: row; align-items: center; \
                 justify-content: space-between; gap: {gap}px; \
                 padding: 0 {px}px; text-align: left; cursor: pointer; \
                 background: {bg}; border: none; \
                 border-bottom: 1px solid {border}; {edge} \
                 font-size: {fs}px; color: {fg};",
                h = height,
                gap = tokens::SPACE_2,
                px = tokens::SPACE_3,
                bg = if is_current { tokens::COLOR_SURFACE_3 } else { "transparent" },
                border = tokens::COLOR_BORDER_CHROME,
                edge = if is_current {
                    format!("border-left: 3px solid {};", tokens::COLOR_TAB_ACTIVE_INDICATOR)
                } else {
                    String::new()
                },
                fs = tokens::FONT_SIZE_MD,
                fg = if row.bundled {
                    tokens::COLOR_TEXT_ON_CHROME
                } else {
                    tokens::COLOR_TEXT_ON_CHROME_SECONDARY
                },
            ),
            onclick: move |evt| {
                evt.stop_propagation();
                let mut next = draft.read().clone();
                if let Some(d) = next.as_mut() {
                    d.style.char_props.font_name = Some(name.clone());
                }
                draft.set(next);
                query.set(String::new());
                open.set(false);
            },
            // Each family previews in its own face, which is the only way to
            // tell two names apart before committing to one.
            span {
                style: format!("font-family: {};", row.name),
                {row.name.clone()}
            }
            if let Some(sub) = row.substitutes_for.clone() {
                span {
                    style: format!(
                        "flex-shrink: 0; font-size: {fs}px; color: {fg};",
                        fs = tokens::FONT_SIZE_XS,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    {sub}
                }
            }
        }
    }
}
