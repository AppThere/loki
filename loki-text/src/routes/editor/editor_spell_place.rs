// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Where the spelling menu goes, once it is root-hosted (Spec 08 T4.1).
//!
//! Separated from the panel's rendering so the three defects the migration is
//! meant to close can be asserted headlessly rather than looked at — and so
//! `editor_spell_panel` keeps the "wires, does not decide" shape the primitive's
//! own modules have.
//!
//! # Status: **rerouted (r63)** — and what that does and does not establish
//!
//! `spell_menu_placement` is called from `editor_spell_popover`, whose request
//! `AtPopoverHost` renders at the app root. So the three defects below are
//! addressed *in the product* and not only in a test: the flip comes from
//! `popover::place`, the clipping ancestor is gone because the host is a child of
//! the app root, and the coordinate spaces now agree.
//!
//! **What is established headlessly:** the placement arithmetic, by the tests in
//! `editor_spell_place_tests.rs`, and that the wiring compiles and type-checks.
//!
//! **What is not:** that the menu appears where it should on a screen. Nothing in
//! this environment can render it. The first observation of the session that can
//! is the **scroll-drift check** — open the same menu with the editor scrolled to
//! different positions — because a correct placement stays pinned to the word
//! while an origin error drifts with the container, and those two are only
//! separable while both are still in view. Judged "working" first, the
//! distinction is gone.
//!
//! # Why the migration fixes all three at once
//!
//! | defect | what changes |
//! | --- | --- |
//! | no bottom-edge collision | `popover::place` flips |
//! | clipped by the editor root | the host is a child of the app root, outside every `overflow` ancestor |
//! | **~41px below the click** | the containing block becomes the app root, whose padding box starts at the window origin — so window coordinates are already correct there |
//!
//! The third is the one worth being explicit about, because it looks like it
//! needs a conversion and does not. The anchor is window-relative (this stack's
//! `client_*`; see `docs/patches.md`). An absolutely-positioned child resolves
//! against its containing block's **padding box**, and the app root has
//! `margin: 0` with no border, so its padding box starts at window `(0, 0)`.
//! Hosting there makes the anchor's coordinate space and the host's the same
//! space. The old ~41px error was the tab bar, contributed by the *editor root*
//! being the containing block instead.
//!
//! # The safe area is the viewport, not the window
//!
//! The app root's padding is the safe-area inset, but absolute children resolve
//! against the padding box — i.e. the **full** window — so a popover at `top: 0`
//! would sit under a notification bar. `PlacementRequest::viewport` is therefore
//! the usable rect, which is exactly what that field documents.

use appthere_ui::components::popover::{Align, MIN_ANCHORED_MENU_PX, PlacementRequest, Rect, Side};

/// Menu width in CSS pixels.
pub(super) const MENU_WIDTH_PX: f32 = 300.0;
/// Maximum height before the menu scrolls.
pub(super) const MENU_MAX_HEIGHT_PX: f32 = 320.0;
/// Distance kept from every viewport edge.
pub(super) const EDGE_MARGIN_PX: f32 = 8.0;
/// A viewport that constrains nothing, for the frame before the window is
/// measured.
///
/// Large enough that no clamp binds and finite so the arithmetic stays
/// well-defined — `f32::MAX` would make `right()` overflow to infinity inside
/// `place`.
const UNBOUNDED_UNTIL_MEASURED: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1.0e6,
    height: 1.0e6,
};

/// Gap between the caret and the menu.
const CARET_GAP_PX: f32 = 4.0;
/// Caret height, so the menu clears the line rather than sitting on it.
const CARET_HEIGHT_PX: f32 = 18.0;

/// Builds the placement request for a right-click at `(click_x, click_y)` in
/// **window** coordinates.
///
/// # No window size, no insets — the host fills the viewport
///
/// Those parameters were here until r63, and taking them was wrong in the way
/// that matters: the window and the safe area are properties of the *host's*
/// coordinate space, and a consumer's only route to a window height is "container
/// metrics plus known chrome" — the ~41px class of arithmetic this whole task
/// exists to delete. `AtPopoverContext::open_resolved` fills
/// `PlacementRequest::viewport` from the measured window, so every consumer gets
/// the same answer and none of them computes it.
///
/// What remains here is what only the consumer knows: where the caret is, how
/// wide the menu is, and that a context menu opens downward from the click with
/// its left edge on it.
#[must_use]
pub(super) fn spell_menu_placement(click_x: f32, click_y: f32) -> PlacementRequest {
    PlacementRequest {
        // A caret, not a control: zero width, one line tall. `Align::Start` then
        // puts the menu's left edge on the click, which is what a context menu
        // does.
        anchor: Rect::new(click_x, click_y, 0.0, CARET_HEIGHT_PX),
        width: MENU_WIDTH_PX,
        height: MENU_MAX_HEIGHT_PX,
        // Overwritten by the host, which owns the viewport. Until it is — the
        // frame before the window sensor reports — this must not *constrain*,
        // because an unmeasured viewport is an absence of information rather than
        // a small screen. A fabricated 1280×800 would have been a fake device,
        // which is what `check-no-hardcoded-viewport-dims` exists to refuse, and
        // a zero rect would clamp the menu to the origin.
        viewport: UNBOUNDED_UNTIL_MEASURED,
        preferred: Side::Below,
        align: Align::Start,
        gap: CARET_GAP_PX,
        margin: EDGE_MARGIN_PX,
        min_anchored_height: MIN_ANCHORED_MENU_PX,
    }
}

#[cfg(test)]
#[path = "editor_spell_place_tests.rs"]
mod tests;
