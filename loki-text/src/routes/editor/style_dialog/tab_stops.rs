// SPDX-License-Identifier: Apache-2.0

//! The **Tab stops** tab: the style's tab-stop table.
//!
//! # Deleting an inherited stop
//!
//! A style that inherits its stops has `tab_stops == None`. Deleting one from
//! the visible list therefore cannot "remove an entry" — there is no local list
//! to remove it from, and editing the parent's list would change every sibling.
//! The first edit **materialises the resolved list locally**, minus the deleted
//! stop, so the change is confined to this style. The tab says so on the line
//! under the table, because it is a data-model consequence rather than a UI
//! detail: after the first delete the style no longer tracks its parent's stops.

use appthere_ui::tokens;
use appthere_ui::{
    AtDialogButton, AtDialogNotice, AtField, AtNoticeTone, DialogPosture, at_control_style,
};
use dioxus::prelude::*;
use loki_doc_model::style::props::tab_stop::TabStop;
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::draft::{fmt_points, parse_points};
use super::fields::{DraftSignal, OpenSignal, full_width, provenance_line};
use super::rows::resolve_row;
use super::tab_stops_cells::{alignment_label, body_cell, header_cell, leader_label};

/// Renders the Tab stops tab body.
pub(super) fn body(
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    // The visible list is the **resolved** one, so an inherited set is shown
    // rather than an empty table over a paragraph that plainly has stops.
    let local = current.style.para_props.tab_stops.clone();
    let stops = local.clone().or_else(|| {
        catalog
            .resolve_para_chain(id, |s| s.para_props.tab_stops.clone())
            .and_then(|r| r.value)
    });
    let stops = stops.unwrap_or_default();
    let is_local = local.is_some();
    let new_stop = current.buffers.new_tab_stop.clone();

    let row = resolve_row(
        catalog,
        id,
        |s| s.para_props.tab_stops.clone(),
        |v: &Vec<TabStop>| fl!("style-dialog-stops-count", count = v.len() as i64),
    );

    rsx! {
        div {
            style: format!(
                "flex: 1; min-width: 0; display: flex; flex-direction: column; \
                 gap: {gap}px; padding: {p}px; overflow-y: auto;",
                gap = tokens::SPACE_4,
                p = tokens::SPACE_5,
            ),

            // ── The table ─────────────────────────────────────────────────────
            div {
                style: format!(
                    "border: 1px solid {border}; border-radius: {r}px; overflow: hidden;",
                    border = tokens::COLOR_BORDER_CHROME,
                    r = tokens::RADIUS_MD,
                ),
                div {
                    style: format!(
                        "display: flex; flex-direction: row; background: {bg}; \
                         border-bottom: 1px solid {border};",
                        bg = tokens::COLOR_SURFACE_2,
                        border = tokens::COLOR_BORDER_CHROME,
                    ),
                    { header_cell(fl!("style-dialog-stops-position"), "flex: 1;") }
                    { header_cell(fl!("style-dialog-stops-alignment"), "flex: 1;") }
                    { header_cell(fl!("style-dialog-stops-leader"), "flex: 1;") }
                    div { style: format!("width: {}px; flex-shrink: 0;", tokens::TOUCH_MIN) }
                }

                if stops.is_empty() {
                    div {
                        style: format!(
                            "padding: {p}px; font-size: {fs}px; color: {fg};",
                            p = tokens::SPACE_4,
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        { fl!("style-dialog-stops-empty") }
                    }
                }

                for (i, stop) in stops.iter().enumerate() {
                    div {
                        key: "stop-{i}",
                        style: format!(
                            "display: flex; flex-direction: row; align-items: center; \
                             min-height: {h}px; border-bottom: 1px solid {border}; \
                             font-size: {fs}px; color: {fg};",
                            h = posture.min_touch_px.max(tokens::TOUCH_MIN),
                            border = tokens::COLOR_BORDER_CHROME,
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        { body_cell(fl!("style-dialog-unit-pt", value = fmt_points(Some(stop.position))), "flex: 1;") }
                        { body_cell(alignment_label(stop.alignment), "flex: 1;") }
                        { body_cell(leader_label(stop.leader), "flex: 1;") }
                        button {
                            aria_label: fl!("style-dialog-stops-remove-aria"),
                            style: format!(
                                "width: {t}px; height: {t}px; flex-shrink: 0; \
                                 display: flex; align-items: center; justify-content: center; \
                                 background: transparent; border: none; cursor: pointer; \
                                 font-size: {fs}px; color: {fg};",
                                t = tokens::TOUCH_MIN,
                                fs = tokens::FONT_SIZE_BODY,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            onclick: move |evt| {
                                evt.stop_propagation();
                                edit_stops(draft, move |list| {
                                    if i < list.len() {
                                        list.remove(i);
                                    }
                                });
                            },
                            "\u{2715}"
                        }
                    }
                }
            }

            // The provenance line for the whole list — stops resolve as one
            // property, so they carry one line rather than one per row.
            {
                row.as_ref()
                    .map(|(source, value)| {
                        provenance_line(
                            source,
                            value.as_deref(),
                            posture,
                            open_style,
                            draft,
                            |d| d.style.para_props.tab_stops = None,
                        )
                    })
                    .unwrap_or_else(|| rsx! {})
            }

            // ── Add a stop ────────────────────────────────────────────────────
            div {
                style: format!(
                    "display: flex; flex-direction: {dir}; align-items: {align}; gap: {gap}px;",
                    dir = if posture.full_screen { "column" } else { "row" },
                    align = if posture.full_screen { "stretch" } else { "flex-end" },
                    gap = tokens::SPACE_3,
                ),
                AtField {
                    label: fl!("style-dialog-stops-new-at"),
                    extra_style: "flex: 1;".to_string(),
                    control: rsx! {
                        div {
                            style: at_control_style(posture.min_touch_px, "width: 100%;"),
                            input {
                                r#type: "text",
                                value: "{new_stop}",
                                style: format!(
                                    "flex: 1; min-width: 0; background: transparent; \
                                     border: none; font-size: {fs}px; color: {fg};",
                                    fs = tokens::FONT_SIZE_MD,
                                    fg = tokens::COLOR_TEXT_ON_CHROME,
                                ),
                                oninput: move |evt| {
                                    let mut draft = draft;
                                    let mut next = draft.read().clone();
                                    if let Some(d) = next.as_mut() {
                                        d.buffers.new_tab_stop = evt.value();
                                    }
                                    draft.set(next);
                                },
                            }
                            span {
                                style: format!(
                                    "flex-shrink: 0; font-size: {fs}px; color: {fg};",
                                    fs = tokens::FONT_SIZE_LABEL,
                                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                ),
                                { fl!("style-dialog-unit-pt-short") }
                            }
                        }
                    },
                }
                AtDialogButton {
                    label: fl!("style-dialog-stops-add"),
                    min_touch_px: posture.min_touch_px,
                    // A stop with no position is not a stop; the button stays
                    // inert rather than adding one at zero.
                    disabled: !matches!(parse_points(&new_stop), Ok(Some(_))),
                    on_click: move |_| add_stop(draft),
                }
                AtDialogButton {
                    label: fl!("style-dialog-stops-clear"),
                    min_touch_px: posture.min_touch_px,
                    disabled: stops.is_empty(),
                    on_click: move |_| edit_stops(draft, |list| list.clear()),
                }
            }

            // The suppression consequence, stated where it happens.
            if !is_local && !stops.is_empty() {
                AtDialogNotice {
                    tone: AtNoticeTone::Caution,
                    extra_style: full_width(posture),
                    message: rsx! { { fl!("style-dialog-stops-materialise-note") } },
                }
            }
        }
    }
}

/// Applies an edit to the style's tab-stop list.
///
/// Materialises the **resolved** list locally first when the style has none of
/// its own, so an edit to an inherited set is confined to this style instead of
/// reaching every sibling through the parent.
fn edit_stops(mut draft: DraftSignal, edit: impl Fn(&mut Vec<TabStop>)) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        let mut list = d.style.para_props.tab_stops.clone().unwrap_or_default();
        edit(&mut list);
        // Stops are positional, and the layout engine walks them in order.
        list.sort_by(|a, b| {
            a.position
                .value()
                .partial_cmp(&b.position.value())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        d.style.para_props.tab_stops = Some(list);
    }
    draft.set(next);
}

/// Adds the stop in the "new stop at" buffer, then clears the buffer.
fn add_stop(mut draft: DraftSignal) {
    let buffer = draft
        .read()
        .as_ref()
        .map(|d| d.buffers.new_tab_stop.clone())
        .unwrap_or_default();
    let Ok(Some(position)) = parse_points(&buffer) else {
        return;
    };
    edit_stops(draft, move |list| {
        // A second stop at the same position is unreachable — the first one
        // consumes the tab — so replace rather than accumulate.
        if let Some(existing) = list
            .iter_mut()
            .find(|s| (s.position.value() - position.value()).abs() < f64::EPSILON)
        {
            existing.position = position;
        } else {
            list.push(TabStop::left(position));
        }
    });
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        d.buffers.new_tab_stop = String::new();
    }
    draft.set(next);
}
