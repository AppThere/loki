// SPDX-License-Identifier: Apache-2.0

//! The Insert table body: size picker beside options at pointer sizes, stacked
//! at Compact.

use appthere_ui::{
    AtCheckRow, AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture,
    at_control_style, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::grid::{drag_grid, steppers};
use super::preview::specimen;
use super::spec::{TableSpec, WidthChoice};

/// Renders the dialog body.
pub(super) fn form(
    open: Signal<Option<TableSpec>>,
    posture: DialogPosture,
    spec: &TableSpec,
) -> Element {
    // Note 23: the drag grid is a pointer affordance, so it disappears at
    // Compact and the steppers take over. Both write the same two numbers.
    let show_grid = !posture.full_screen;

    rsx! {
        div {
            style: format!(
                "flex: 1; min-width: 0; display: flex; flex-direction: column; \
                 gap: {gap}px; padding: {p}px; overflow-y: auto;",
                gap = tokens::SPACE_5,
                p = tokens::SPACE_5,
            ),

            // ── Size ──────────────────────────────────────────────────────────
            div {
                style: format!(
                    "display: flex; flex-direction: {dir}; gap: {gap}px; align-items: {align};",
                    dir = if show_grid { "row" } else { "column" },
                    gap = tokens::SPACE_5,
                    align = if show_grid { "flex-start" } else { "stretch" },
                ),
                if show_grid {
                    { drag_grid(open, spec, posture) }
                }
                div {
                    style: "flex: 1; min-width: 0;",
                    { steppers(open, spec) }
                }
            }

            // ── Caption ───────────────────────────────────────────────────────
            AtField {
                label: fl!("table-dialog-caption"),
                control: rsx! {
                    div {
                        style: at_control_style(posture.min_touch_px, "width: 100%;"),
                        input {
                            r#type: "text",
                            value: "{spec.caption}",
                            placeholder: fl!("table-dialog-caption-placeholder"),
                            style: format!(
                                "flex: 1; min-width: 0; background: transparent; border: none; \
                                 font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_BODY,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            oninput: move |evt| {
                                let mut open = open;
                                let mut next = open.read().clone();
                                if let Some(s) = next.as_mut() {
                                    s.caption = evt.value();
                                }
                                open.set(next);
                            },
                        }
                    }
                },
                footnote: rsx! {
                    div {
                        style: format!(
                            "display: flex; align-items: center; gap: {gap}px; \
                             font-size: {fs}px; color: {fg};",
                            gap = tokens::SPACE_1,
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        span { "\u{21B3}" }
                        span { { fl!("table-dialog-caption-note") } }
                    }
                },
            }

            // ── Structure ─────────────────────────────────────────────────────
            div {
                style: "display: flex; flex-direction: column;",
                AtCheckRow {
                    checked: spec.header_row,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("table-dialog-header-row"),
                    label: rsx! { { fl!("table-dialog-header-row") } },
                    on_toggle: move |v: bool| {
                        let mut open = open;
                        let mut next = open.read().clone();
                        if let Some(s) = next.as_mut() {
                            s.header_row = v;
                        }
                        open.set(next);
                    },
                }
                // A header row *is* the repeat mechanism: `thead` repeats on
                // every page by definition in both ODF and OOXML, so this is
                // stated rather than offered as a second switch that could
                // disagree with the first.
                AtCheckRow {
                    checked: spec.header_row,
                    disabled: true,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("table-dialog-repeat-header"),
                    label: rsx! { { fl!("table-dialog-repeat-header") } },
                    on_toggle: move |_| {},
                }
                // No per-cell header flag exists in `CellProps`, and rows carry
                // no keep-together flag, so neither control is drawn as live.
                // TODO(table-model-header-column): add a cell-level header flag
                // to `CellProps` and a row-level keep-together to `Row`, then
                // wire these two through the ODF/OOXML mappers.
                AtCheckRow {
                    checked: false,
                    disabled: true,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("table-dialog-header-column"),
                    label: rsx! { { fl!("table-dialog-header-column") } },
                    on_toggle: move |_| {},
                }
                AtCheckRow {
                    checked: false,
                    disabled: true,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("table-dialog-rows-break"),
                    label: rsx! { { fl!("table-dialog-rows-break") } },
                    on_toggle: move |_| {},
                }
            }

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                message: rsx! { { fl!("table-dialog-unsupported") } },
            }

            // ── Width ─────────────────────────────────────────────────────────
            AtField {
                label: fl!("table-dialog-width"),
                control: rsx! {
                    AtSegmented {
                        options: WidthChoice::ALL.iter().map(|w| w.label()).collect::<Vec<_>>(),
                        selected: spec.width.index(),
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            let mut open = open;
                            let mut next = open.read().clone();
                            if let Some(s) = next.as_mut() {
                                s.width = WidthChoice::from_index(idx);
                            }
                            open.set(next);
                        },
                    }
                },
            }

            // ── Preview ───────────────────────────────────────────────────────
            { specimen(spec) }
        }
    }
}
