// SPDX-License-Identifier: Apache-2.0

//! The drag grid and the ± steppers (design notes 23 and 25).
//!
//! # Two affordances, one pair of numbers
//!
//! The grid is a **pointer** affordance: it trades precision for speed, and it
//! needs a pointer to sweep. It shrinks at Medium and disappears at Compact,
//! replaced by ±48 px steppers. Both write the same `rows` and `cols` — there
//! is no second source of truth, so a size chosen either way survives a window
//! resize that swaps which control is on screen.

use appthere_ui::{DialogPosture, at_field_label_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::spec::{MAX_COLS, MAX_GRID_COLS, MAX_GRID_ROWS, MAX_ROWS, TableSpec};

/// Cell edge length in the Expanded grid, in CSS px.
const CELL_PX: f32 = 26.0;
/// Cell edge length in the shrunk Medium grid.
const CELL_MEDIUM_PX: f32 = 18.0;
/// Gap between grid cells.
const CELL_GAP_PX: f32 = 3.0;
/// Stepper button edge — the design's ±48 px touch target.
const STEPPER_PX: f32 = 48.0;

/// The drag grid. Clicking a cell sets the size to that cell's coordinates.
///
/// `aria_label` names the whole grid; each cell carries the size it selects, so
/// a screen-reader user hears "4 rows by 3 columns" rather than "button".
pub(super) fn drag_grid(
    mut open: Signal<Option<TableSpec>>,
    spec: &TableSpec,
    posture: DialogPosture,
) -> Element {
    let cell = if posture.preview_docked {
        CELL_PX
    } else {
        CELL_MEDIUM_PX
    };
    let (rows, cols) = (spec.rows, spec.cols);

    rsx! {
        div {
            role: "grid",
            aria_label: fl!("table-dialog-grid-aria"),
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px;",
                gap = CELL_GAP_PX,
            ),
            for r in 1..=MAX_GRID_ROWS {
                div {
                    key: "row-{r}",
                    style: format!(
                        "display: flex; flex-direction: row; gap: {gap}px;",
                        gap = CELL_GAP_PX,
                    ),
                    for c in 1..=MAX_GRID_COLS {
                        button {
                            key: "cell-{r}-{c}",
                            aria_label: fl!(
                                "table-dialog-grid-cell-aria",
                                rows = r as i64,
                                cols = c as i64
                            ),
                            style: format!(
                                "width: {cell}px; height: {cell}px; padding: 0; \
                                 border-radius: {radius}px; cursor: pointer; \
                                 border: 1px solid {border}; background: {bg};",
                                radius = tokens::RADIUS_SM,
                                border = tokens::COLOR_BORDER_CHROME,
                                // Fill every cell up and left of the pointer, so
                                // the grid reads as the table it will make.
                                bg = if r <= rows && c <= cols {
                                    tokens::COLOR_TAB_ACTIVE_INDICATOR
                                } else {
                                    tokens::COLOR_SURFACE_3
                                },
                            ),
                            onclick: move |evt| {
                                evt.stop_propagation();
                                let mut next = open.read().clone();
                                if let Some(s) = next.as_mut() {
                                    s.rows = r;
                                    s.cols = c;
                                }
                                open.set(next);
                            },
                        }
                    }
                }
            }
            div {
                style: format!(
                    "text-align: center; font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_META,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("table-dialog-size-summary", rows = rows as i64, cols = cols as i64) }
            }
        }
    }
}

/// A ±  stepper over one axis.
pub(super) fn stepper(
    label: String,
    value: usize,
    max: usize,
    mut open: Signal<Option<TableSpec>>,
    set: impl Fn(&mut TableSpec, usize) + Copy + 'static,
) -> Element {
    let mut step = move |delta: isize| {
        let mut next = open.read().clone();
        if let Some(s) = next.as_mut() {
            let updated = value.saturating_add_signed(delta).clamp(1, max);
            set(s, updated);
        }
        open.set(next);
    };

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px; flex: 1;",
                gap = tokens::SPACE_2,
            ),
            div { style: at_field_label_style(), {label.clone()} }
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; gap: {gap}px;",
                    gap = tokens::SPACE_3,
                ),
                button {
                    aria_label: fl!("table-dialog-decrease-aria", axis = label.clone()),
                    disabled: value <= 1,
                    style: stepper_button_style(value <= 1),
                    onclick: move |evt| {
                        evt.stop_propagation();
                        step(-1);
                    },
                    "\u{2212}"
                }
                div {
                    style: format!(
                        "flex: 1; height: {h}px; box-sizing: border-box; display: flex; \
                         align-items: center; justify-content: center; \
                         background: {bg}; border: 1px solid {border}; \
                         border-radius: {r}px; font-size: {fs}px; color: {fg};",
                        h = STEPPER_PX,
                        bg = tokens::COLOR_SURFACE_2,
                        border = tokens::COLOR_BORDER_CHROME,
                        r = tokens::RADIUS_MD,
                        fs = tokens::FONT_SIZE_SUBHEADING,
                        fg = tokens::COLOR_TEXT_ON_CHROME,
                    ),
                    {value.to_string()}
                }
                button {
                    aria_label: fl!("table-dialog-increase-aria", axis = label),
                    disabled: value >= max,
                    style: stepper_button_style(value >= max),
                    onclick: move |evt| {
                        evt.stop_propagation();
                        step(1);
                    },
                    "+"
                }
            }
        }
    }
}

/// Both steppers, side by side.
pub(super) fn steppers(open: Signal<Option<TableSpec>>, spec: &TableSpec) -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px;",
                gap = tokens::SPACE_5,
            ),
            { stepper(fl!("table-dialog-rows"), spec.rows, MAX_ROWS, open, |s, v| s.rows = v) }
            { stepper(fl!("table-dialog-columns"), spec.cols, MAX_COLS, open, |s, v| s.cols = v) }
        }
    }
}

/// The ± button style. Disabled at the clamp bounds rather than silently
/// no-oping, so the ceiling is visible instead of merely felt.
fn stepper_button_style(disabled: bool) -> String {
    format!(
        "width: {w}px; height: {h}px; flex-shrink: 0; box-sizing: border-box; \
         display: flex; align-items: center; justify-content: center; \
         background: {bg}; border: 1px solid {border}; border-radius: {r}px; \
         cursor: {cursor}; font-size: {fs}px; color: {fg};",
        w = STEPPER_PX + 4.0,
        h = STEPPER_PX,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_MD,
        cursor = if disabled { "default" } else { "pointer" },
        fs = tokens::FONT_SIZE_HEADING,
        fg = if disabled {
            tokens::COLOR_ICON_DISABLED
        } else {
            tokens::COLOR_TEXT_ON_CHROME
        },
    )
}
