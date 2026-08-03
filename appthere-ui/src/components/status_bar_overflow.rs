// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The status bar's overflow ("More") popover (Spec 08 T7.1) — where items
//! dropped by [`super::status_bar_items`]'s priority order go.
//!
//! Modelled directly on [`super::ribbon::overflow_menu`], deliberately: it is
//! the same problem at the other end of the window, and the ribbon's version
//! already carries the two findings that cost a defect each — the menu must be
//! hosted by `AtPopoverHost` (Blitz builds no stacking contexts, so an in-place
//! menu's controls are dead under a root-sibling backdrop), and a key the
//! overlay consumes but cannot act on is a focus trap, so both focus moves
//! dismiss.
//!
//! # `Side::Above`, for the same reason and one more
//!
//! The status bar is the bottom-most chrome in the window, so a menu asking for
//! `Below` would be flipped on every single open — making the flip path the one
//! every user takes, and a regression in flipping present as "the menu is
//! off-screen". Asking for what we want is the one that fails visibly.

use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::popover::{
    Align, KeyAction, OverlayKind, PlacementRequest, PopoverId, PopoverRequest, Rect, Role, Side,
    MIN_ANCHORED_MENU_PX,
};
use crate::tokens;

/// Identifies the status overflow to the popover singleton rule.
pub(super) const STATUS_OVERFLOW_POPOVER_ID: PopoverId = PopoverId(0x0_5442);

/// Clear space between the More button and the menu.
const ANCHOR_GAP_PX: f32 = 4.0;

/// Closest the menu may come to a viewport edge.
const EDGE_MARGIN_PX: f32 = 8.0;

/// The menu's own padding, both axes.
const MENU_PADDING_PX: f32 = tokens::SPACE_2;

/// Height allowed per row, for the placement estimate. The primitive clamps to
/// the viewport and the menu's own layout finally sizes it.
const ROW_HEIGHT_PX: f32 = 28.0;

/// Floor for the menu width. A menu narrower than this is hard to hit even when
/// its rows are short, and an item moved into an unhittable menu has been
/// removed rather than relocated.
const MIN_MENU_WIDTH_PX: f32 = 160.0;

/// The viewport the host substitutes its measured one for.
const UNBOUNDED_UNTIL_MEASURED: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: f32::MAX,
    height: f32::MAX,
};

/// One row of the overflow menu: the item's label, its accessible name, and its
/// action when it has one.
///
/// `on_click` is `None` for the readouts (word count, language, collaborators),
/// which are text in the bar and text here. A row that looks pressable and does
/// nothing is the shape this suite already avoids, so a `None` row renders as a
/// plain line rather than a dead button.
#[derive(Clone)]
pub(super) struct StatusOverflowRow {
    pub label: String,
    pub aria_label: String,
    pub on_click: Option<EventHandler<()>>,
}

impl PartialEq for StatusOverflowRow {
    /// Compared by the two strings only. `EventHandler` has no meaningful
    /// equality, and the rows are rebuilt from props every render, so comparing
    /// the handler would make every render a change.
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
            && self.aria_label == other.aria_label
            && self.on_click.is_some() == other.on_click.is_some()
    }
}

/// Menu size from the rows it will hold.
///
/// Derived from the same declared width the fit engine used to decide these
/// items did not fit — read for a second purpose rather than restated, so the
/// two cannot disagree.
#[must_use]
fn menu_size(rows: &[StatusOverflowRow]) -> (f32, f32) {
    let widest = rows
        .iter()
        .map(|r| crate::responsive::estimate_label_px(&r.label, tokens::FONT_SIZE_XS))
        .fold(0.0_f32, f32::max);
    let width = widest.max(MIN_MENU_WIDTH_PX) + 2.0 * MENU_PADDING_PX;
    let height = (rows.len() as f32).mul_add(ROW_HEIGHT_PX, 2.0 * MENU_PADDING_PX);
    (width, height)
}

/// Where the menu sits relative to the More button: above (the bar is at the
/// window bottom) and end-aligned (the trigger is at the strip's right edge, so
/// a start-aligned menu would hang off it and be shifted back on every open).
#[must_use]
fn menu_placement(anchor: Rect, size: (f32, f32)) -> PlacementRequest {
    PlacementRequest {
        anchor,
        width: size.0,
        height: size.1,
        viewport: UNBOUNDED_UNTIL_MEASURED,
        preferred: Side::Above,
        align: Align::End,
        gap: ANCHOR_GAP_PX,
        margin: EDGE_MARGIN_PX,
        min_anchored_height: MIN_ANCHORED_MENU_PX,
    }
}

/// The popover request for the status overflow menu.
///
/// `rows` is cloned into the content closure rather than borrowed: the closure
/// runs during the **host's** render, which outlives this call.
pub(super) fn status_overflow_request(
    rows: Vec<StatusOverflowRow>,
    anchor_rect: Rect,
    anchor_el: Option<Rc<MountedData>>,
    open: Signal<bool>,
) -> PopoverRequest {
    let size = menu_size(&rows);
    let content_rows = rows;

    PopoverRequest {
        id: STATUS_OVERFLOW_POPOVER_ID,
        placement: menu_placement(anchor_rect, size),
        on_dismiss: Rc::new(move || {
            // Re-bound inside: `Signal` is `Copy`, which gives an `Fn` closure
            // the mutable handle it cannot capture.
            let mut open = open;
            open.set(false);
        }),
        anchor: anchor_el,
        on_outside_move: None,
        kind: OverlayKind::Dismissible,
        role: Role::Panel,
        on_key: Some(Rc::new(move |action: KeyAction| {
            // `Dismiss` never arrives — the host performs it. The two focus
            // moves would otherwise be consumed and dropped, trapping the
            // keyboard inside a menu it could not leave.
            if matches!(
                action,
                KeyAction::FocusNextControl | KeyAction::FocusPrevControl
            ) {
                let mut open = open;
                open.set(false);
            }
        })),
        content: Rc::new(move || {
            rsx! {
                div {
                    role: "group",
                    style: format!(
                        "display: flex; flex-direction: column; gap: {gap}px; \
                         padding: {pad}px; background: {bg}; \
                         border: 1px solid {border}; border-radius: {radius}px;",
                        gap = tokens::SPACE_1,
                        pad = MENU_PADDING_PX,
                        bg = tokens::COLOR_SURFACE_2,
                        border = tokens::COLOR_BORDER_CHROME,
                        radius = tokens::RADIUS_MD,
                    ),
                    for row in content_rows.iter() {
                        { overflow_row(row) }
                    }
                }
            }
        }),
    }
}

/// One menu row: a button when the item has an action, plain text when it does
/// not.
///
/// # Touch target
///
/// An actionable row is [`tokens::TOUCH_MIN`] tall, meeting WCAG 2.5.8 — the
/// menu is not height-constrained the way the 24 px status bar is, so the
/// bar's documented shortfall does not follow the item in here.
fn overflow_row(row: &StatusOverflowRow) -> Element {
    let text_style = format!(
        "font-size: {size}px; color: {fg}; white-space: nowrap;",
        size = tokens::FONT_SIZE_XS,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    );
    match row.on_click {
        Some(handler) => rsx! {
            button {
                key: "{row.aria_label}",
                "aria-label": row.aria_label.clone(),
                style: format!(
                    "{text_style} min-height: {h}px; background: transparent; \
                     border: none; cursor: pointer; text-align: left; \
                     padding: 0 {pad}px; display: flex; align-items: center;",
                    h = tokens::TOUCH_MIN,
                    pad = tokens::SPACE_2,
                ),
                onclick: move |_| handler.call(()),
                "{row.label}"
            }
        },
        None => rsx! {
            span {
                key: "{row.aria_label}",
                style: format!("{text_style} padding: 0 {pad}px;", pad = tokens::SPACE_2),
                "{row.label}"
            }
        },
    }
}

#[path = "status_bar_overflow_trigger.rs"]
mod trigger;
pub(super) use trigger::AtStatusOverflow;
