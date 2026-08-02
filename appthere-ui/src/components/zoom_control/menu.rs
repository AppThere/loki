// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The zoom preset menu — its popover request and its rendering (Spec 08 T5.4).
//!
//! Split from `mod.rs` to keep both under the 300-line ceiling, and because the
//! control and its menu are separable in the way the file boundary suggests: the
//! control owns the requested zoom, the menu owns which row the keyboard is on.

use std::rc::Rc;

use dioxus::prelude::*;

use super::field::{erase_last, push_digit, starts_typed_zoom, typed_zoom_field};
use super::rows::{next_zoom_row, prev_zoom_row, zoom_rows, ZoomCommands, ZoomRow};
use super::{AtZoomLabels, ZOOM_POPOVER_ID};
use crate::components::popover::{
    Align, KeyAction, OverlayKind, PlacementRequest, PopoverRequest, Rect, Role, Side,
    MIN_ANCHORED_MENU_PX,
};
use crate::components::zoom::{parse_zoom_percent, ZOOM_PRESETS_PERCENT};
use crate::tokens::colors::{COLOR_BORDER_CHROME, COLOR_SURFACE_PAGE, COLOR_TEXT_PRIMARY};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_1, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::FONT_SIZE_BODY;

/// Background of the row the keyboard is on. Light, because the menu surface is
/// light — the `COLOR_*_HOVER` tokens are chrome-dark and read as a hole here.
/// TODO(tokens-light-hover): add a light-surface hover token and use it.
const ACTIVE_ROW_BG: &str = "#EDEDED";

/// Everything the menu needs that the control owns.
#[derive(Clone)]
pub(super) struct ZoomMenuContext {
    /// The zoom currently requested, so the menu can mark its row.
    pub percent: u32,
    /// Which computed rows are offered.
    pub commands: ZoomCommands,
    pub labels: AtZoomLabels,
    pub on_change: EventHandler<u32>,
    pub on_fit_width: EventHandler<()>,
    pub on_fit_page: EventHandler<()>,
    pub on_actual_size: EventHandler<()>,
}

impl ZoomMenuContext {
    /// Runs a row's action. **The single activation path**, called by both the
    /// pointer and the keyboard — the Phase 4 rule (L08-029), which here would
    /// otherwise let Enter pick a different preset from the one clicked.
    pub(super) fn activate(&self, row: ZoomRow) {
        match row {
            ZoomRow::Preset(i) => {
                if let Some(p) = ZOOM_PRESETS_PERCENT.get(i).copied() {
                    self.on_change.call(p);
                }
            }
            ZoomRow::FitWidth => self.on_fit_width.call(()),
            ZoomRow::FitPage => self.on_fit_page.call(()),
            ZoomRow::ActualSize => self.on_actual_size.call(()),
        }
    }

    /// This row's label.
    fn label(&self, row: ZoomRow) -> String {
        match row {
            ZoomRow::Preset(i) => ZOOM_PRESETS_PERCENT
                .get(i)
                .map_or_else(String::new, |p| format!("{p}%")),
            ZoomRow::FitWidth => self.labels.fit_width.clone(),
            ZoomRow::FitPage => self.labels.fit_page.clone(),
            ZoomRow::ActualSize => self.labels.actual_size.clone(),
        }
    }
}

/// Menu width — wide enough for "Actual size" without wrapping.
const MENU_WIDTH_PX: f32 = 200.0;

/// The tallest the menu asks to be: every row of the longest form (nine presets
/// plus three commands) plus its padding. The primitive shrinks it to fit and
/// scrolls the overflow, so this is a request rather than an assumption about
/// the window.
const MENU_MAX_HEIGHT_PX: f32 = 12.0 * TOUCH_MIN + 2.0 * SPACE_1;

/// Gap between the readout and the menu.
const ANCHOR_GAP_PX: f32 = 4.0;

/// Minimum distance kept from every window edge.
const EDGE_MARGIN_PX: f32 = 8.0;

/// The viewport stand-in used before the host measures the real one — the same
/// convention `recent_menu` uses, and for the same reason: the request is built
/// where the window size is not known, and the host substitutes the measured
/// rect before placing.
const UNBOUNDED_UNTIL_MEASURED: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: f32::MAX,
    height: f32::MAX,
};

/// Where the menu sits relative to the readout button.
///
/// **Above**, because the zoom control lives in the status bar at the bottom of
/// the window. Asking for `Below` and letting the primitive flip would work, but
/// it would make the flip path the one every open takes — so a regression in
/// flipping would present as "the zoom menu is off-screen" rather than as an
/// edge case, and the preferred side would be documentation of an intent nobody
/// holds.
pub(super) fn zoom_menu_placement(anchor: Rect) -> PlacementRequest {
    PlacementRequest {
        anchor,
        width: MENU_WIDTH_PX,
        height: MENU_MAX_HEIGHT_PX,
        viewport: UNBOUNDED_UNTIL_MEASURED,
        preferred: Side::Above,
        align: Align::End,
        gap: ANCHOR_GAP_PX,
        margin: EDGE_MARGIN_PX,
        min_anchored_height: MIN_ANCHORED_MENU_PX,
    }
}

/// The popover request for the zoom menu.
pub(super) fn zoom_menu_request(
    ctx: ZoomMenuContext,
    anchor_rect: Rect,
    anchor_el: Option<Rc<MountedData>>,
    active: Signal<Option<String>>,
    open: Signal<bool>,
    typed: Signal<Option<String>>,
) -> PopoverRequest {
    let rows = zoom_rows(ctx.commands);

    let key_ctx = ctx.clone();
    let key_rows = rows.clone();
    let render_ctx = ctx;

    PopoverRequest {
        id: ZOOM_POPOVER_ID,
        placement: zoom_menu_placement(anchor_rect),
        on_dismiss: Rc::new(move || {
            // Re-bound inside, for the `Fn`-not-`FnMut` reason above.
            let mut open = open;
            open.set(false);
        }),
        anchor: anchor_el,
        // The rows carry no hover tint, so nothing consumes an outside move.
        // `None` rather than a no-op: an empty callback cannot be told apart
        // from a forgotten one.
        on_outside_move: None,
        kind: OverlayKind::Dismissible,
        role: Role::Menu,
        on_key: Some(Rc::new(move |action: KeyAction| {
            let mut active = active;
            // `peek`, not `read`: this runs from an event handler, and a
            // subscription taken here would tie whatever is rendering to the row
            // the keyboard last touched.
            let current = active.peek().clone();
            // While the field is open it owns the keyboard: arrows would
            // otherwise move a highlight the reader cannot see past the field,
            // and Enter would activate a preset instead of submitting.
            if typed.peek().is_some() {
                let mut typed = typed;
                let mut open = open;
                let current = typed.peek().clone().unwrap_or_default();
                match action {
                    KeyAction::Activate => {
                        if let Some(p) = parse_zoom_percent(&current) {
                            key_ctx.on_change.call(p);
                            typed.set(None);
                            open.set(false);
                        }
                        // An entry that does not parse leaves the field as it is,
                        // so the reader can correct it. Closing would discard
                        // what they typed and look like it was accepted.
                    }
                    KeyAction::Typeahead(c) if starts_typed_zoom(c) => {
                        typed.set(Some(push_digit(&current, c)));
                    }
                    KeyAction::Erase => typed.set(Some(erase_last(&current))),
                    KeyAction::Dismiss => typed.set(None),
                    _ => {}
                }
                return;
            }

            match action {
                KeyAction::Next => {
                    active.set(next_zoom_row(&key_rows, current.as_deref()).map(|r| r.key()));
                }
                KeyAction::Prev => {
                    active.set(prev_zoom_row(&key_rows, current.as_deref()).map(|r| r.key()));
                }
                KeyAction::First => active.set(key_rows.first().map(|r| r.key())),
                KeyAction::Last => active.set(key_rows.last().map(|r| r.key())),
                KeyAction::Activate => {
                    if let Some(row) = current
                        .as_deref()
                        .and_then(|k| key_rows.iter().find(|r| r.key() == k))
                    {
                        key_ctx.activate(*row);
                        // `Signal` is `Copy`, so the captured copy is re-bound
                        // mutably here; writing through the capture itself would
                        // need `FnMut`, which the host cannot hold.
                        let mut open = open;
                        open.set(false);
                    }
                }
                // Typeahead over "25%", "50%" … would match the first character
                // of a *label*, when the character a reader types is the first
                // digit of the **value** they want. So a digit starts the typed
                // field with that digit in it — the same observation, used the
                // other way round. See `field`.
                KeyAction::Typeahead(c) if starts_typed_zoom(c) => {
                    let mut typed = typed;
                    // Every later key is routed here too — `route_key` sends the
                    // whole keyboard to this closure while the menu is open — so
                    // the field's buffer is edited from the same place rather
                    // than from an element whose text does not survive a render.
                    let current = typed.peek().clone().unwrap_or_default();
                    typed.set(Some(push_digit(&current, c)));
                }
                _ => {}
            }
        })),
        // `active` is read **inside** the closure so the read subscribes the
        // host and an arrow key repaints the menu. Read outside, the highlight
        // would be frozen at open time.
        content: Rc::new(move || {
            menu_content(&render_ctx, &rows, active.read().clone(), open, typed)
        }),
    }
}

/// The menu's rows.
fn menu_content(
    ctx: &ZoomMenuContext,
    rows: &[ZoomRow],
    active: Option<String>,
    open: Signal<bool>,
    typed: Signal<Option<String>>,
) -> Element {
    let items = rows.to_vec();
    rsx! {
        div {
            style: format!(
                "background: {bg}; border: 1px solid {bd}; border-radius: {r}px; \
                 padding: {p}px 0; min-width: 180px;",
                bg = COLOR_SURFACE_PAGE,
                bd = COLOR_BORDER_CHROME,
                r = RADIUS_MD,
                p = SPACE_1,
            ),
            {typed_zoom_field(typed, typed.read().is_some())}

            for row in items {
                {
                    let key = row.key();
                    let is_active = active.as_deref() == Some(key.as_str());
                    let is_current = row.preset_percent() == Some(ctx.percent);
                    let label = ctx.label(row);
                    let ctx = ctx.clone();
                    rsx! {
                        button {
                            key: "{key}",
                            style: format!(
                                "display: flex; align-items: center; width: 100%; \
                                 min-height: {t}px; padding: 0 {ph}px; border: none; \
                                 border-radius: {r}px; cursor: pointer; text-align: left; \
                                 font-size: {fs}px; color: {fg}; background: {bg}; \
                                 font-weight: {fw};",
                                t = TOUCH_MIN,
                                ph = SPACE_3,
                                r = RADIUS_SM,
                                fs = FONT_SIZE_BODY,
                                fg = COLOR_TEXT_PRIMARY,
                                bg = if is_active { ACTIVE_ROW_BG } else { "transparent" },
                                fw = if is_current { "700" } else { "400" },
                            ),
                            onclick: move |_| {
                                let mut open = open;
                                ctx.activate(row);
                                open.set(false);
                            },
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}
