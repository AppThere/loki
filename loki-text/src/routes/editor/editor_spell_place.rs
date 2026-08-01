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
//! this environment can render it. The first observation of the next session is
//! the **scroll-drift check**, and its procedure has to be stated precisely,
//! because the obvious phrasing is ambiguous between two experiments that answer
//! different questions:
//!
//! > **Open the menu fresh at several editor scroll positions** — same word,
//! > different container offsets, closing and re-opening each time.
//!
//! **Not** "scroll while the menu is open". The per-frame anchor driver
//! (`popover::interaction::on_anchor_change`) is not wired, so scrolling with the
//! menu open leaves it where it was — and that is the *same visible symptom* as a
//! coordinate-space error: menu and word separating as the document moves. The
//! two are indistinguishable that way, and the wrong reading reports an origin
//! defect that does not exist.
//!
//! Opening fresh isolates the conversion from the driver: each open re-reads the
//! anchor, so a correct placement lands on the word at *every* scroll offset,
//! while an origin error is displaced by the container's offset and therefore
//! grows with it.
//!
//! ## The headless half is now tested (r73)
//!
//! The sitting's question splits in two, and only one half needs a screen:
//! **(1)** does the platform's `client_*` track the word as the container
//! scrolls — Blitz's business, not testable here; **(2)** does *our* code
//! introduce a scroll term — entirely ours, and the half that has been wrong
//! twice. [`spell_menu_anchor`] makes the coordinate choice a named function, and
//! `editor_spell_place_tests` asserts (2) directly: the anchor tracks the window
//! figure exactly across a scroll sweep, no constant survives anywhere in the
//! path, and four mutations — the tile-local pair, a constant, a `+41.0`, and a
//! mixed pair — all fail.
//!
//! **What that changes about the sitting.** It does not replace it; it makes its
//! outcome diagnostic. Two of the three readings below are now *pre-excluded* by
//! test, so if the menu drifts on screen with these green, the defect is in the
//! platform half, and re-reading our arithmetic will not find it.
//!
//! ## Reading the result — the shape of the error names the defect
//!
//! Written here rather than worked out at the time, because all three look like
//! "the menu is in the wrong place" and the difference decides whether anything
//! is wrong at all:
//!
//! | what you see | what it is |
//! | --- | --- |
//! | offset by the **same** amount at every scroll position | a coordinate-space error that survived r63 — a constant, so a fixed chrome height |
//! | offset **grows** with the container's scroll | an origin error: the anchor and the host disagree about where zero is |
//! | lands correctly, then **separates only while scrolling with it open** | the absent per-frame driver. **Expected, not a regression** — D-15 specifies it and it is unwritten |
//!
//! The third row is why the procedure says *open fresh*: scrolling with the menu
//! open produces the same visible separation as row two, so an experiment that
//! mixes them reports an origin defect that does not exist.
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
use loki_renderer::TileContext;

/// The anchor point for a right-click, in **window** coordinates.
///
/// # A one-line function, because this choice has been wrong twice
///
/// [`TileContext`] carries two coordinate pairs from the same event, and they
/// mean different things: `x_pt`/`y_pt` are **tile-local layout points**, used to
/// hit-test which word was clicked, and `client_x`/`client_y` are
/// **window-relative CSS pixels**, used to place the menu. Reading the wrong pair
/// compiles, type-checks, and puts the menu at a plausible-looking offset — r42
/// and r43 are both instances, and this stack additionally *swaps* the DOM's
/// `client_*` and `page_*` senses (see `docs/patches.md`), so the names cannot be
/// trusted to disambiguate.
///
/// Making the selection a named function with a test means a mutation to the
/// other pair fails rather than reads plausibly. That is the half of the
/// scroll-drift check that does not need a screen: **it establishes that no
/// scroll term enters the anchor**, which is what makes placement independent of
/// the editor's scroll offset. The other half — that the platform's `client_*`
/// really do track the word as the container scrolls — is the sitting.
///
/// # Why no scroll term is correct, now that the host is at the root
///
/// Window coordinates need adjusting only when the containing block scrolls with
/// the content. It does not: the popover host is a child of the app root, which
/// is `100vh` with `overflow: hidden`. An absolutely-positioned child resolves
/// against that root's padding box, which starts at window `(0, 0)`, so the
/// anchor's space and the host's are the same space and the correct adjustment is
/// none.
#[must_use]
pub(super) fn spell_menu_anchor(ctx: &TileContext) -> (f32, f32) {
    (ctx.client_x, ctx.client_y)
}

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
