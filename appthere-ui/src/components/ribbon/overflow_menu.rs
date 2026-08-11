// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The ribbon overflow ("More") menu as a popover request (Spec 08 I-28).
//!
//! # What was wrong, and why a higher z-index could never have fixed it
//!
//! The menu used to render in place — `position: absolute` inside the strip,
//! `z-index: 41`, with the app's root backdrop at 40 catching outside clicks.
//! The comment justifying that said 41 beats 40 so the menu's own controls stay
//! clickable. It does not. Blitz builds **no stacking contexts**: `paint_children`
//! sorts each parent's own layout children by `z_index()` among siblings, and
//! hit-tests that list in reverse. This menu is deep inside `Router`; the
//! backdrop is a root sibling. A descendant's z never meets a root sibling's, so
//! the backdrop was hit first and every control in the menu was dead while it
//! was raised.
//!
//! Hosting it fixes the cause: `AtPopoverHost` renders the menu **and** its
//! backdrop as root siblings, in one order, with one lifetime.
//!
//! # `Role::Panel`, and the strain that choice exposes
//!
//! The content is a set of ribbon groups — rows of icon buttons. `Panel` is the
//! role that describes that: arrows, Enter, Space and characters pass through to
//! whichever button has focus, which is what a button container wants and what
//! `Menu` would swallow into `Next`/`Prev` that this content has no item model to
//! perform.
//!
//! **But `route_key(Panel, Tab)` is `FocusNextControl`, and nothing in the
//! workspace performs it.** It has no producer *or* consumer outside
//! `interaction.rs` — this is its first real consumer, and it arrives to find
//! the action unimplementable for the same reason `AdvanceFocusPastAnchor` is:
//! `RenderedElementBacking::set_focus` takes a `bool`, so "focus the next
//! control" cannot be expressed. Reported rather than absorbed, per I-28's own
//! instruction; see the `advance_focus_past|focus_next_node` register row, which
//! now has a second waiting consumer.
//!
//! A key consumed and dropped is a **trap** — focus is inside the menu and Tab
//! would do nothing — so `on_key` maps both focus-moves to a dismissal. That is
//! the same outcome `Role::Menu`'s `DismissAndAdvance` would give, chosen
//! explicitly by the consumer that knows it cannot move focus internally, rather
//! than inherited from a role that misdescribes the content.

use std::rc::Rc;

use dioxus::prelude::*;

use super::group::AtRibbonGroup;
use super::groups::RibbonGroupSpec;
use crate::components::popover::{
    Align, KeyAction, OverlayKind, PlacementRequest, PopoverId, PopoverRequest, Rect, Role, Side,
    MIN_ANCHORED_MENU_PX,
};
use crate::responsive::GroupCollapse;
use crate::tokens;

/// Identifies the overflow menu to the popover singleton rule.
pub(super) const OVERFLOW_POPOVER_ID: PopoverId = PopoverId(0x0_5044);

/// Identifies the §11 per-group Partial submenu. One id for all chips: the
/// strip opens at most one submenu at a time (the singleton rule would
/// enforce it anyway; a single id makes the intent structural).
pub(super) const PARTIAL_POPOVER_ID: PopoverId = PopoverId(0x0_5045);

/// Clear space between the More button and the menu.
const ANCHOR_GAP_PX: f32 = 4.0;

/// Closest the menu may come to a viewport edge.
const EDGE_MARGIN_PX: f32 = 8.0;

/// The menu's own padding, both axes — it wraps the groups.
const MENU_PADDING_PX: f32 = tokens::SPACE_2;

/// Fallback width when no overflowed group declares one.
///
/// Reached only if every metric is zero, which `estimate_group_metrics` cannot
/// produce; a menu of literal zero width would be invisible and unclickable,
/// which is indistinguishable from the defect this task fixes.
const MIN_MENU_WIDTH_PX: f32 = 160.0;

/// Height allowed per overflowed group, for the placement estimate.
///
/// The groups render at [`GroupCollapse::Full`] — a button row plus its label —
/// and this is that height. The primitive only needs an estimate: it clamps to
/// the viewport and the menu's own layout is what finally sizes it.
const GROUP_HEIGHT_PX: f32 = 72.0;

/// The viewport the host substitutes its measured one for.
const UNBOUNDED_UNTIL_MEASURED: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: f32::MAX,
    height: f32::MAX,
};

/// Menu size from the groups it will hold.
///
/// Derived rather than a constant, because the numbers already exist:
/// `GroupMetrics::full_px` is what the cascade used to decide these groups did
/// **not** fit, so it is the same measurement, read for a second purpose. A
/// magic width here would be a second statement of how wide a group is, and the
/// two would disagree the first time a tab declared an unusual group.
#[must_use]
pub(super) fn overflow_menu_size(overflowed: &[&RibbonGroupSpec]) -> (f32, f32) {
    let widest = overflowed
        .iter()
        .map(|g| g.metrics.full_px)
        .fold(0.0_f32, f32::max);
    let width = widest.max(MIN_MENU_WIDTH_PX) + 2.0 * MENU_PADDING_PX;
    let count = overflowed.len() as f32;
    let height = count.mul_add(GROUP_HEIGHT_PX, 2.0 * MENU_PADDING_PX);
    (width, height)
}

/// Where the menu sits relative to the More button.
///
/// **Above**, because the ribbon is at the window bottom. Asking for `Below` and
/// letting the primitive flip would work and would make the flip path the one
/// every open takes — so a regression in flipping would present as "the overflow
/// menu is off-screen" rather than as an edge case. Same reasoning as the zoom
/// menu, which sits below this one.
///
/// `Align::End`: the More button is the last thing in the strip, so a menu
/// aligned to its start would hang off the right edge and be shifted back by the
/// primitive on every open.
#[must_use]
pub(super) fn overflow_menu_placement(anchor: Rect, size: (f32, f32)) -> PlacementRequest {
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

/// The popover request for the overflow menu — also reused by the §11
/// per-group Partial submenu (same content model: whole groups rendered Full
/// in a hosted panel), which passes its own `id` and close handler.
///
/// `overflowed` is cloned into the content closure rather than borrowed: the
/// closure is invoked during the **host's** render, which outlives this call.
pub(super) fn overflow_menu_request(
    id: PopoverId,
    overflowed: Vec<RibbonGroupSpec>,
    anchor_rect: Rect,
    anchor_el: Option<Rc<MountedData>>,
    close: Rc<dyn Fn()>,
) -> PopoverRequest {
    let size = overflow_menu_size(&overflowed.iter().collect::<Vec<_>>());
    let content_groups = overflowed;
    let key_close = Rc::clone(&close);

    PopoverRequest {
        id,
        placement: overflow_menu_placement(anchor_rect, size),
        on_dismiss: close,
        anchor: anchor_el,
        // The groups' buttons tint themselves from their own
        // `onmouseenter`/`onmouseleave`, so nothing here needs to learn the
        // pointer left. `None` rather than a no-op: an empty callback cannot be
        // told apart from a forgotten one.
        on_outside_move: None,
        kind: OverlayKind::Dismissible,
        role: Role::Panel,
        on_key: Some(Rc::new(move |action: KeyAction| {
            // See the module note. `Dismiss` never arrives here — the host
            // performs it — so the only actions this can receive are the two
            // focus moves, and dropping them would trap the keyboard inside a
            // menu it could not leave.
            if matches!(
                action,
                KeyAction::FocusNextControl | KeyAction::FocusPrevControl
            ) {
                key_close();
            }
        })),
        content: Rc::new(move || {
            rsx! {
                div {
                    style: format!(
                        "display: flex; flex-direction: column; gap: {gap}px; \
                         padding: {pad}px; background: {bg}; \
                         border: 1px solid {border}; border-radius: {radius}px;",
                        gap = tokens::SPACE_2,
                        pad = MENU_PADDING_PX,
                        bg = tokens::COLOR_SURFACE_2,
                        border = tokens::COLOR_BORDER_CHROME,
                        radius = tokens::RADIUS_MD,
                    ),
                    // Overflowed groups render in Full form inside the menu —
                    // stacked vertically, so no right divider.
                    for spec in content_groups.iter() {
                        AtRibbonGroup {
                            key: "{spec.aria_label}",
                            label: spec.label.clone(),
                            aria_label: spec.aria_label.clone(),
                            collapse: GroupCollapse::Full,
                            show_divider: false,
                            {spec.content.clone()}
                        }
                    }
                }
            }
        }),
    }
}

#[cfg(test)]
#[path = "overflow_menu_tests.rs"]
mod tests;
