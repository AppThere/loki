// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The Recent Documents row menu, as a popover consumer (Spec 08 T4.2, I-08).
//!
//! # What this replaces, and why the old shape could not be fixed in place
//!
//! The menu was rendered **inside its own row**, expanding it and pushing every
//! row below it down. Two consequences the primitive removes:
//!
//! - **The list scrolls, so the menu was clipped.** `AtHomeTab`'s recent list
//!   carries `overflow-y: auto`; an inline menu on the last visible row is cut
//!   by it. No `z-index` escapes a clip, which is why T4.2 is the consumer that
//!   forced the popup itself to be root-hosted rather than merely its backdrop.
//! - **Opening a menu moved the content under the pointer.** Expanding a row
//!   reflows the rows beneath it, so the thing the user was aiming at moves
//!   while they are aiming at it.
//!
//! # The identity hazard is real here, not hypothetical
//!
//! The list's open-menu state was `Signal<Option<usize>>` — **an index**. The
//! rows are keyed by path, so a `documents` prop that changes while a menu is
//! open leaves the menu attached to whatever document now sits at that index,
//! and `Delete file` is one of the actions. `wiring::AnchorKey` and
//! `on_anchor_identity` exist for exactly this, and they had no caller until
//! now; the key here is a hash of the document's path, which is content-derived
//! rather than positional (see [`RecentMenuTarget`]).
//!
//! # Touch target
//!
//! Every action row carries `min-height: TOUCH_MIN` (44 px, WCAG 2.5.8), and the
//! menu asks the primitive for `MIN_ANCHORED_MENU_PX` — two rows — so a viewport
//! too short to show a list that reads as a list gets a floored, scrolling menu
//! rather than a single row that looks like the whole thing.

use std::hash::{Hash, Hasher};
use std::rc::Rc;

use dioxus::prelude::*;

use crate::components::popover::{
    use_popover_anchor, Align, AnchorKey, OverlayKind, PlacementRequest, PopoverId, PopoverRequest,
    Rect, Side, MIN_ANCHORED_MENU_PX,
};
use crate::tokens::colors::{
    COLOR_BORDER_CHROME, COLOR_STATUS_ERROR_TEXT, COLOR_SURFACE_PAGE, COLOR_TEXT_PRIMARY,
};
use crate::tokens::spacing::{RADIUS_MD, RADIUS_SM, SPACE_1, SPACE_2, SPACE_3, TOUCH_MIN};
use crate::tokens::typography::FONT_SIZE_BODY;
use crate::{use_safe_area, use_window_size};

/// Identifies the Recent Documents menu to the popover singleton rule.
const RECENT_POPOVER_ID: PopoverId = PopoverId(0x8_EC0D);

/// Menu width in CSS pixels. Wide enough for the longest action label at the
/// body size without wrapping, which is what makes a row's height predictable
/// and therefore makes `MIN_ANCHORED_MENU_PX` mean two *whole* rows.
const MENU_WIDTH_PX: f32 = 240.0;
/// Maximum height before the menu scrolls: three rows plus its own padding.
const MENU_MAX_HEIGHT_PX: f32 = 3.0 * TOUCH_MIN + 2.0 * SPACE_2;
/// Gap between the ⋮ button and the menu.
const ANCHOR_GAP_PX: f32 = 4.0;
/// Distance kept from every viewport edge.
const EDGE_MARGIN_PX: f32 = 8.0;

/// A viewport that constrains nothing, for the frame before the window is
/// measured. The host overwrites it; finite so `right()` stays well-defined.
const UNBOUNDED_UNTIL_MEASURED: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1.0e6,
    height: 1.0e6,
};

/// Which document a menu belongs to, and where its trigger sits.
///
/// # The key is the path, deliberately, and the index rides along
///
/// `key` answers *which document* and is a hash of its path — content-derived,
/// so it survives the list reordering. `index` is what the parent's
/// `EventHandler<usize>` callbacks still speak, so it is carried rather than
/// trusted: every use of it is guarded by an identity check against `key` first.
///
/// Keeping both is not two sources for one fact (L08-029) — they answer
/// different questions, and the whole point is that the answer to "which
/// document" must not be derived from the answer to "which position".
#[derive(Clone, PartialEq)]
pub(super) struct RecentMenuTarget {
    /// Content-derived identity of the document this menu was opened for.
    pub key: AnchorKey,
    /// Position at open time — usable only after an identity check.
    pub index: usize,
    /// The ⋮ button's rect in window coordinates, from `get_client_rect`.
    pub anchor: Rect,
}

/// Hashes a document path into an [`AnchorKey`].
///
/// A hash rather than the path itself because `AnchorKey` is a `u64` — chosen so
/// the identity comparison is cheap enough to run on every anchor change. A
/// collision would attach a menu to the wrong document, which is why the
/// *comparison* is only ever used to decide whether to dismiss: a false "same"
/// is bounded by the same guard as a stale index, and a false "different" costs
/// a dismissal.
#[must_use]
pub(super) fn key_for_path(path: &str) -> AnchorKey {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    AnchorKey(hasher.finish())
}

/// Where the menu goes, given the ⋮ button's rect.
///
/// Below the trigger and right-aligned to it, which is where a row's overflow
/// menu belongs: the button sits at the row's right edge, so `Align::End` keeps
/// the menu inside the list rather than hanging off it. `place` flips and shifts
/// from there.
#[must_use]
pub(super) fn recent_menu_placement(anchor: Rect) -> PlacementRequest {
    PlacementRequest {
        anchor,
        width: MENU_WIDTH_PX,
        height: MENU_MAX_HEIGHT_PX,
        viewport: UNBOUNDED_UNTIL_MEASURED,
        preferred: Side::Below,
        align: Align::End,
        gap: ANCHOR_GAP_PX,
        margin: EDGE_MARGIN_PX,
        min_anchored_height: MIN_ANCHORED_MENU_PX,
    }
}

/// Labels and callbacks the menu needs, bundled so the props stay readable.
#[derive(Clone, PartialEq)]
pub(super) struct RecentMenuActions {
    pub remove_label: String,
    pub delete_label: String,
    pub open_copy_label: String,
    pub on_remove: EventHandler<usize>,
    pub on_delete: EventHandler<usize>,
    pub on_open_copy: EventHandler<usize>,
}

/// Props for [`RecentMenuPopover`].
#[derive(Props, Clone, PartialEq)]
pub(super) struct RecentMenuPopoverProps {
    /// The open menu's target. The component is mounted only while this is
    /// `Some` — see the module docs on ADR-0013.
    pub target: RecentMenuTarget,
    /// Paths of the documents currently in the list, in order. The identity
    /// check reads this rather than a count: a list that changed length is not
    /// the interesting case, a list where *this* path moved or vanished is.
    pub paths: Vec<String>,
    pub actions: RecentMenuActions,
    /// Clears the parent's open-menu state.
    pub on_dismiss: EventHandler<()>,
}

/// Hands the Recent Documents menu to the popover host.
///
/// A `#[component]` mounted at the boundary (ADR-0013): it owns a hook scope, so
/// it can push the request from an effect rather than writing a signal during a
/// render, and `use_popover_anchor` installs the unmount cleanup that r66 was
/// created by omitting.
///
/// # Touch target
///
/// Renders nothing itself; the rows it builds carry the 44 px minimum.
#[component]
pub(super) fn RecentMenuPopover(props: RecentMenuPopoverProps) -> Element {
    let popover = use_popover_anchor(RECENT_POPOVER_ID);
    let window = use_window_size();
    let insets = use_safe_area();

    use_effect(move || {
        let Some(anchor) = popover else {
            return;
        };
        let target = props.target.clone();
        let actions = props.actions.clone();
        let on_dismiss = props.on_dismiss;
        // The identity check, before geometry and before any action can fire.
        // `IdentityCheck::Recycled` is the case that matters: the list reordered
        // under an open menu, so the index the callbacks speak now names a
        // different document — and one of the actions deletes a file.
        let under_anchor = props.paths.get(target.index).map(|path| key_for_path(path));
        if crate::components::popover::on_anchor_identity(target.key, under_anchor).must_dismiss() {
            on_dismiss.call(());
            return;
        }
        let index = target.index;
        anchor.open(
            PopoverRequest {
                id: RECENT_POPOVER_ID,
                placement: recent_menu_placement(target.anchor),
                on_dismiss: Rc::new(move || on_dismiss.call(())),
                // No hover tinting in this menu — the rows use a static
                // background — so the outside-move signal has no consumer here.
                // Left `None` rather than wired to a no-op: an empty callback is
                // indistinguishable from a forgotten one.
                on_outside_move: None,
                kind: OverlayKind::Dismissible,
                content: Rc::new(move || menu_content(actions.clone(), index, on_dismiss)),
            },
            window,
            insets,
        );
    });

    rsx! {}
}

/// The menu's rows.
///
/// Each action closes the menu *before* calling its handler, because two of the
/// three change the list the menu is anchored inside — removing an entry moves
/// every row below it, and a menu left open would then be attached to a
/// different document by the identity check's own definition.
fn menu_content(actions: RecentMenuActions, index: usize, on_dismiss: EventHandler<()>) -> Element {
    let row_style = |fg: &'static str| {
        format!(
            "background: transparent; border: none; text-align: left; width: 100%; \
             padding: {p}px {ph}px; min-height: {touch}px; cursor: pointer; \
             font-size: {size}px; color: {fg}; border-radius: {r}px; \
             box-sizing: border-box;",
            p = SPACE_2,
            ph = SPACE_3,
            touch = TOUCH_MIN,
            size = FONT_SIZE_BODY,
            r = RADIUS_SM,
        )
    };
    let close_then = move |handler: EventHandler<usize>| {
        move |_| {
            on_dismiss.call(());
            handler.call(index);
        }
    };
    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; gap: {gap}px; \
                 width: 100%; box-sizing: border-box; padding: {p}px; \
                 background: {bg}; border: 1px solid {border}; \
                 border-radius: {r}px;",
                gap = SPACE_1,
                p = SPACE_2,
                bg = COLOR_SURFACE_PAGE,
                border = COLOR_BORDER_CHROME,
                r = RADIUS_MD,
            ),
            button {
                style: row_style(COLOR_TEXT_PRIMARY),
                onclick: close_then(actions.on_remove),
                "{actions.remove_label}"
            }
            button {
                style: row_style(COLOR_STATUS_ERROR_TEXT),
                onclick: close_then(actions.on_delete),
                "{actions.delete_label}"
            }
            button {
                style: row_style(COLOR_TEXT_PRIMARY),
                onclick: close_then(actions.on_open_copy),
                "{actions.open_copy_label}"
            }
        }
    }
}

#[cfg(test)]
#[path = "recent_menu_tests.rs"]
mod tests;
