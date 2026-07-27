// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Where the spelling menu goes, once it is root-hosted (Spec 08 T4.1).
//!
//! Separated from the panel's rendering so the three defects the migration is
//! meant to close can be asserted headlessly rather than looked at — and so
//! `editor_spell_panel` keeps the "wires, does not decide" shape the primitive's
//! own modules have.
//!
//! # Status: **pending reroute — all three defects are live in the product**
//!
//! Read this before reading the tests. `spell_menu_placement` has no caller:
//! `editor_spell_panel` still renders in place and still positions itself, so
//! **every one of the three defects below is present in the shipping app today**
//! — including the ~41px offset, which is the one that looks like ordinary
//! behaviour on screen.
//!
//! What the six tests in `editor_spell_place_tests.rs` establish is that *this
//! function* would place correctly **if it were called**. That is a real result
//! — it is what makes the reroute a wiring change rather than a design one — but
//! it is not a fix, and six green tests next to a defect list are exactly the
//! arrangement in which a later reader concludes it is. The reroute is the thing
//! that converts them; nothing else does.
//!
//! **The status is mechanical, not a promise.** The three suppressions in this
//! file are `expect(dead_code)`, not `allow`: once a **reachable** caller
//! appears, each expectation goes unfulfilled and the build fails under CI's
//! `-D warnings`, quoting the `reason` as the failure's own note. So the reroute
//! cannot land while this section still says pending — whoever wires it has to
//! come here and delete it.
//!
//! *Reachable* is not pedantry: dead-code analysis is transitive, so a caller
//! that is itself dead leaves the expectation fulfilled and silent. Checked, by
//! wiring a call from `spelling_panel` and reading the message rustc emits — the
//! real reroute is a call from a live component, which does trip it.
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

use appthere_ui::SafeAreaInsets;
use appthere_ui::components::popover::{Align, MIN_ANCHORED_MENU_PX, PlacementRequest, Rect, Side};

/// Menu width in CSS pixels.
pub(super) const MENU_WIDTH_PX: f32 = 300.0;
/// Maximum height before the menu scrolls.
pub(super) const MENU_MAX_HEIGHT_PX: f32 = 320.0;
/// Distance kept from every viewport edge.
pub(super) const EDGE_MARGIN_PX: f32 = 8.0;
/// Gap between the caret and the menu.
#[expect(
    dead_code,
    reason = "pending T4.1 reroute — delete this and the status section above when `editor_spell_panel` renders into `AtPopoverHost`"
)]
const CARET_GAP_PX: f32 = 4.0;
/// Caret height, so the menu clears the line rather than sitting on it.
#[expect(
    dead_code,
    reason = "pending T4.1 reroute — delete this and the status section above when `editor_spell_panel` renders into `AtPopoverHost`"
)]
const CARET_HEIGHT_PX: f32 = 18.0;

/// Builds the placement request for a right-click at `(click_x, click_y)` in
/// **window** coordinates.
///
/// # Not yet wired — and that is the whole of what remains
///
/// `editor_spell_panel` still positions itself, so this function is exercised
/// only by its tests. Said plainly rather than left to be discovered, because it
/// is the same shape as `max_servable_zoom_permille`: **tests that guard a
/// function nobody calls**. Its six cases show that all three defects *would* be
/// closed here, and they will keep passing while the panel keeps all three.
///
/// What wires it is one change and it is all-or-nothing: the panel must render
/// **into `AtPopoverHost`** rather than in place. Using this placement while
/// still rendering inside the editor root would fix the flip and leave the
/// ~41px offset — a half-migration, which Spec 08 r45 records as the outcome
/// hardest to tell from a fix.
///
/// `window` is the full window size; `insets` are the safe-area insets, which
/// bound the usable viewport.
// TODO(t4.1-popover): render the panel into `AtPopoverHost` and call this.
// Deliberately not called yet — see the module's status section. The alternative
// to this suppression was a half-migration, which is worse than an honest gap;
// `expect` rather than `allow` so wiring a caller breaks the build here.
#[expect(
    dead_code,
    reason = "pending T4.1 reroute — delete this and the status section above when `editor_spell_panel` renders into `AtPopoverHost`"
)]
#[must_use]
pub(super) fn spell_menu_placement(
    click_x: f32,
    click_y: f32,
    window: (f32, f32),
    insets: SafeAreaInsets,
) -> PlacementRequest {
    PlacementRequest {
        // A caret, not a control: zero width, one line tall. `Align::Start` then
        // puts the menu's left edge on the click, which is what a context menu
        // does.
        anchor: Rect::new(click_x, click_y, 0.0, CARET_HEIGHT_PX),
        width: MENU_WIDTH_PX,
        height: MENU_MAX_HEIGHT_PX,
        viewport: Rect::new(
            insets.left,
            insets.top,
            (window.0 - insets.left - insets.right).max(0.0),
            (window.1 - insets.top - insets.bottom).max(0.0),
        ),
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
