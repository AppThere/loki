// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A ribbon split button: a primary action beside a chevron that opens an
//! anchored menu of related actions (Spec 04; usage audit §4 — the Write tab's
//! Document group).
//!
//! # Two full-size buttons, deliberately
//!
//! The classic split button narrows its chevron to a sliver, which fails the
//! 44 px touch minimum this design system holds every interactive element to
//! (WCAG 2.5.8). Here the primary action and the menu trigger are both
//! [`AtRibbonIconButton`]s — same hit target, same hover treatment — sitting
//! flush in one group. The chevron carries its own `aria_label`, so a screen
//! reader hears two distinct actions rather than one button with a secret half.
//!
//! # Touch target
//!
//! Both buttons are 44 × 44 logical pixels ([`crate::tokens::TOUCH_MIN`]), and
//! every menu row carries `min-height: TOUCH_MIN`.
//!
//! # The menu is a popover consumer
//!
//! The rows render through [`crate::components::popover`]'s host (root-hosted,
//! backdropped, singleton), with the same one-table row model the Recent
//! Documents menu established: the pointer and the keyboard walk the same
//! `Vec`, so they cannot disagree about what a row does.

use std::rc::Rc;

use dioxus::prelude::*;

use super::button::AtRibbonIconButton;
use crate::components::icons::{AtIcon, LUCIDE_CHEVRON_DOWN};
use crate::components::popover::{
    use_popover_anchor, Align, DismissCause, KeyAction, OverlayKind, PlacementRequest, PopoverId,
    PopoverRequest, Rect, Role, Side, MIN_ANCHORED_MENU_PX,
};
use crate::tokens::spacing::{SPACE_2, TOUCH_MIN};
use crate::{use_safe_area, use_window_size};

/// Menu width in CSS pixels — matches the Recent Documents menu, for the same
/// reason: wide enough that no expected label wraps, so row height (and with it
/// `MIN_ANCHORED_MENU_PX`) stays predictable.
const MENU_WIDTH_PX: f32 = 240.0;
/// Gap between the chevron and the menu.
const ANCHOR_GAP_PX: f32 = 4.0;
/// Distance kept from every viewport edge.
const EDGE_MARGIN_PX: f32 = 8.0;
/// Menu height cap before it scrolls: six rows plus padding (the New menu's
/// six templates are the longest expected list).
const MENU_MAX_HEIGHT_PX: f32 = 6.0 * TOUCH_MIN + 2.0 * SPACE_2;
/// Background of the row the keyboard is on — the same light-surface literal
/// the Recent menu uses.
/// TODO(tokens-light-hover): add a light-surface hover token and use it here.
const ACTIVE_ROW_BG: &str = "#EDEDED";

/// One row of a split-button menu.
#[derive(Clone, PartialEq)]
pub struct SplitMenuItem {
    /// Stable identifier handed to `on_item` — never displayed.
    pub id: String,
    /// The row's visible (localised) label.
    pub label: String,
}

/// The row after `current`, wrapping; `None` (no keyboard selection yet) goes
/// to the first row.
#[must_use]
pub(super) fn next_row(current: Option<usize>, count: usize) -> usize {
    match current {
        Some(row) => (row + 1) % count.max(1),
        None => 0,
    }
}

/// The row before `current`, wrapping; `None` goes to the last row, so the
/// first Up reaches the bottom of the menu in one press.
#[must_use]
pub(super) fn prev_row(current: Option<usize>, count: usize) -> usize {
    match current {
        Some(0) | None => count.saturating_sub(1),
        Some(row) => row - 1,
    }
}

/// Where the menu goes: below the chevron, right-aligned to it (the trigger is
/// the group's right edge), flipping and shifting from there.
#[must_use]
pub(super) fn split_menu_placement(anchor: Rect) -> PlacementRequest {
    PlacementRequest {
        anchor,
        width: MENU_WIDTH_PX,
        height: MENU_MAX_HEIGHT_PX,
        // Finite placeholder; the host clamps to the measured window.
        viewport: Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0e6,
            height: 1.0e6,
        },
        preferred: Side::Below,
        align: Align::End,
        gap: ANCHOR_GAP_PX,
        margin: EDGE_MARGIN_PX,
        min_anchored_height: MIN_ANCHORED_MENU_PX,
    }
}

/// Props for [`AtRibbonSplitButton`].
#[derive(Props, Clone, PartialEq)]
pub struct AtRibbonSplitButtonProps {
    /// Identifies this instance's menu to the popover singleton rule. Each
    /// mounted split button needs its own value.
    pub popover_id: u64,
    /// The primary action's accessible name.
    pub aria_label: String,
    /// The primary action's Lucide icon path data.
    pub icon_path: String,
    /// Whether the primary action is disabled (the menu stays available —
    /// e.g. Save disabled on a clean document must not lock away Save As).
    #[props(default)]
    pub is_disabled: bool,
    /// The chevron's accessible name (e.g. "New from template…").
    pub menu_aria_label: String,
    /// The menu's rows, in display **and** keyboard order.
    pub items: Vec<SplitMenuItem>,
    /// The primary action.
    pub on_main: EventHandler<()>,
    /// A row was chosen (by pointer or keyboard), carrying its `id`.
    pub on_item: EventHandler<String>,
}

/// A ribbon split button (see the module docs).
///
/// # Touch target
///
/// Both buttons are 44 × 44 logical pixels (WCAG 2.5.8); menu rows are at
/// least 44 px tall.
#[component]
pub fn AtRibbonSplitButton(props: AtRibbonSplitButtonProps) -> Element {
    let popover = use_popover_anchor(PopoverId(props.popover_id));
    let window = use_window_size();
    let insets = use_safe_area();

    let mut anchor_el = use_signal(|| Option::<Rc<MountedData>>::None);
    let mut trigger = use_signal(|| Option::<MountedEvent>::None);
    // The open menu's anchor rect; `Some` while the menu is up (ADR-0013: the
    // popover push happens in the effect below, keyed off this state).
    let mut menu_target = use_signal(|| Option::<Rect>::None);
    // Keyboard row; `None` until a key arrives so a pointer-open paints no
    // selection nobody asked for.
    let active = use_signal(|| Option::<usize>::None);

    let items = props.items.clone();
    let on_item = props.on_item;
    use_effect(move || {
        let Some(anchor) = popover else {
            return;
        };
        let Some(rect) = *menu_target.read() else {
            // Mirrors `AtZoomControl`'s effect: closing is a state change too,
            // and this is the only place that can turn it into a `dismiss()`.
            // Without it, a row's own `on_dismiss` (which only resets
            // `menu_target`) never reaches the host's `ctx.open`/`ctx.resolved`
            // — `AtPopoverContext::dismiss_with` (the cause-carrying path a row
            // click takes) calls the consumer's `on_dismiss` but does not clear
            // that state itself, so the backdrop is left mounted, eating every
            // click in the app.
            anchor.dismiss();
            return;
        };
        let menu_target_reset = menu_target;
        let rows = items.clone();
        let key_rows = items.clone();
        let count = rows.len();
        let dismiss_activated: Rc<dyn Fn()> =
            Rc::new(move || anchor.dismiss_with(DismissCause::Activated));
        let key_dismiss = Rc::clone(&dismiss_activated);
        let key_on_item = on_item;
        anchor.open(
            PopoverRequest {
                id: PopoverId(props.popover_id),
                placement: split_menu_placement(rect),
                // `Signal` is `Copy`: re-bind the captured copy mutably inside
                // the `Fn` closure (the host cannot hold `FnMut`).
                on_dismiss: Rc::new(move || {
                    let mut menu_target_reset = menu_target_reset;
                    menu_target_reset.set(None);
                }),
                anchor: anchor_el.peek().clone(),
                on_outside_move: None,
                kind: OverlayKind::Dismissible,
                role: Role::Menu,
                on_key: Some(Rc::new(move |action: KeyAction| {
                    let mut active = active;
                    let current = *active.peek();
                    match action {
                        KeyAction::Next => active.set(Some(next_row(current, count))),
                        KeyAction::Prev => active.set(Some(prev_row(current, count))),
                        KeyAction::First => active.set(Some(0)),
                        KeyAction::Last => active.set(Some(count.saturating_sub(1))),
                        KeyAction::Activate => {
                            if let Some(item) = current.and_then(|row| key_rows.get(row)) {
                                // Close first: choosing a row may replace the
                                // ribbon under the menu (New/Open navigate).
                                key_dismiss();
                                key_on_item.call(item.id.clone());
                            }
                        }
                        _ => {}
                    }
                })),
                // `active` is read inside the closure so the host subscribes
                // and arrow keys repaint the highlight.
                content: Rc::new(move || {
                    menu_rows(&rows, on_item, &dismiss_activated, *active.read())
                }),
            },
            window,
            insets,
        );
    });

    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center;",
            AtRibbonIconButton {
                aria_label:  props.aria_label.clone(),
                is_active:   false,
                is_disabled: props.is_disabled,
                on_click:    move |()| props.on_main.call(()),
                AtIcon { path_d: props.icon_path.clone() }
            }
            AtRibbonIconButton {
                aria_label:  props.menu_aria_label.clone(),
                is_active:   menu_target.read().is_some(),
                is_disabled: false,
                on_mounted:  move |evt: MountedEvent| { trigger.set(Some(evt)); },
                on_click:    move |()| {
                    if menu_target.peek().is_some() {
                        menu_target.set(None);
                        return;
                    }
                    let Some(evt) = trigger.peek().clone() else {
                        return;
                    };
                    anchor_el.set(Some(evt.data()));
                    spawn(async move {
                        // Async: `get_client_rect` round-trips to the event
                        // loop. Read at open time rather than kept fresh — the
                        // anchor-change subscription is the host's job.
                        if let Ok(r) = evt.get_client_rect().await {
                            menu_target.set(Some(Rect {
                                x: r.origin.x as f32,
                                y: r.origin.y as f32,
                                width: r.size.width as f32,
                                height: r.size.height as f32,
                            }));
                        }
                    });
                },
                AtIcon { path_d: LUCIDE_CHEVRON_DOWN.to_string() }
            }
        }
    }
}

// The row renderer, split for the 300-line ceiling.
#[path = "split_button_rows.rs"]
mod rows;
use rows::menu_rows;

#[cfg(test)]
#[path = "split_button_tests.rs"]
mod tests;
