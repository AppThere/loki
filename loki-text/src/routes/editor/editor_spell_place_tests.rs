// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The three defects the migration closes, asserted rather than looked at.
//!
//! Each names the pre-migration behaviour it replaces, with the measurement that
//! established it, so a later reader can tell a fix from a coincidence.
//!
//! # The viewport comes from the host's own rule
//!
//! `spell_menu_placement` no longer takes a window size: the host fills the
//! viewport (r63). So these tests call `usable_viewport` — the *same* function the
//! host calls — rather than computing window-minus-insets themselves, which would
//! be a second derivation of the rule under test (L08-029).
//!
//! # Rerouted (r63) — so these now guard code that runs
//!
//! Until r63 this header said the opposite, and had to: `spell_menu_placement`
//! had no caller, so every assertion below was a property of a function outside
//! the product. `editor_spell_popover` calls it now, and `AtPopoverHost` renders
//! the result.
//!
//! **What that changes and what it does not.** These tests establish the
//! placement *arithmetic*, and now that the arithmetic is reachable they also
//! constrain the product. They still cannot see a screen: whether the menu
//! appears where this says it should is the scroll-drift check's business, and
//! that check has to run before the migration is judged working — see the module
//! docs for why the two outcomes stop being separable afterwards.

use appthere_ui::SafeAreaInsets;
use appthere_ui::components::popover::{Side, place, usable_viewport};

use super::spell_menu_placement;

/// A 1280×800 desktop window with no insets.
const WINDOW: (f32, f32) = (1280.0, 800.0);

/// A request as the host would resolve it: the consumer's anchor and
/// preferences, with the viewport filled by the shared rule.
fn placed_at(
    x: f32,
    y: f32,
    insets: SafeAreaInsets,
) -> appthere_ui::components::popover::PlacementRequest {
    let mut req = spell_menu_placement(x, y);
    req.viewport = usable_viewport(
        Some((f64::from(WINDOW.0), f64::from(WINDOW.1))),
        insets,
        req.viewport,
    );
    req
}

fn desktop() -> SafeAreaInsets {
    SafeAreaInsets::default()
}

/// A phone in portrait: a status bar above, a gesture strip below.
fn android() -> SafeAreaInsets {
    SafeAreaInsets {
        top: 34.0,
        bottom: 24.0,
        left: 0.0,
        right: 0.0,
    }
}

/// **Defect 3, the one that would have survived a screen session.** The menu
/// rendered ~41px below the click, because window coordinates were used against
/// a containing block that started below the tab bar (40px + 1px border).
///
/// Root-hosted, the containing block's padding box starts at the window origin,
/// so the anchor's space and the host's space are the same. The menu's top must
/// be the click plus the deliberate caret gap and height — and nothing else.
#[test]
fn the_menu_top_is_the_click_plus_only_the_deliberate_gap() {
    let req = placed_at(400.0, 300.0, desktop());
    let p = place(req);
    assert_eq!(
        p.side,
        Side::Below,
        "precondition: room below at this click"
    );
    let deliberate = req.anchor.height + req.gap;
    assert!(
        (p.rect.y - (300.0 + deliberate)).abs() < 0.001,
        "menu top {} is not the click 300 plus the {deliberate}px caret gap — a \
         residual offset means the coordinate spaces still differ (it was ~41px \
         before the migration, the tab bar's height)",
        p.rect.y,
    );
    assert!(
        (p.rect.x - 400.0).abs() < 0.001,
        "menu left {} is not the click 400",
        p.rect.x,
    );
}

/// **Defect 1.** A right-click on the last line of a page put 298px of a 320px
/// menu below the fold, because viewport height was never a parameter.
#[test]
fn a_click_near_the_bottom_opens_upward_and_stays_on_screen() {
    let req = placed_at(400.0, 760.0, desktop());
    let room_below = req.viewport.bottom() - req.anchor.bottom() - req.gap - req.margin;
    assert!(
        room_below < req.height,
        "fixture no longer creates the condition: {room_below}px below for a {}px \
         menu",
        req.height,
    );
    let p = place(req);
    assert_eq!(
        p.side,
        Side::Above,
        "the only way to fit is above the caret"
    );
    assert!(
        p.rect.is_inside(req.viewport),
        "menu {:?} escaped viewport {:?}",
        p.rect,
        req.viewport,
    );
}

/// The horizontal case the pre-migration code did handle, kept so the fix is
/// shown not to have regressed it.
#[test]
fn a_click_near_the_right_edge_stays_on_screen() {
    let req = placed_at(1250.0, 300.0, desktop());
    assert!(
        req.anchor.x + req.width > req.viewport.right() - req.margin,
        "fixture must push the aligned menu past the right edge",
    );
    let p = place(req);
    assert!(p.rect.is_inside(req.viewport), "{:?}", p.rect);
    assert!(p.shifted);
}

/// **The safe area is the viewport.** Absolute children of the app root resolve
/// against its *padding* box — the full window — so without this the menu would
/// sit under a status bar or a gesture strip.
#[test]
fn the_menu_stays_clear_of_the_system_insets() {
    let insets = android();
    // A click as close to each edge as the content area allows.
    for (x, y) in [(2.0_f32, 2.0_f32), (1270.0, 790.0)] {
        let req = placed_at(x, y, insets);
        let p = place(req);
        assert!(
            p.rect.y >= insets.top,
            "menu top {} is under the status bar ({}px)",
            p.rect.y,
            insets.top,
        );
        assert!(
            p.rect.bottom() <= WINDOW.1 - insets.bottom,
            "menu bottom {} is under the gesture strip",
            p.rect.bottom(),
        );
    }
}

/// Containment holds across the whole window, not only at the corners someone
/// thought to check.
#[test]
fn the_menu_is_on_screen_wherever_the_click_lands() {
    for insets in [desktop(), android()] {
        for cx in 0..=64 {
            for cy in 0..=40 {
                let req = placed_at(cx as f32 * 20.0, cy as f32 * 20.0, insets);
                let p = place(req);
                assert!(
                    p.rect.is_inside(req.viewport),
                    "menu {:?} escaped {:?} for a click at ({}, {})",
                    p.rect,
                    req.viewport,
                    cx * 20,
                    cy * 20,
                );
            }
        }
    }
}

/// The menu must never cover the word it is offering to correct.
#[test]
fn the_menu_never_covers_the_caret_it_belongs_to() {
    for cy in 0..=40 {
        let req = placed_at(400.0, cy as f32 * 20.0, desktop());
        let p = place(req);
        assert!(
            !p.rect.covers_vertically_open(req.anchor),
            "menu {:?} covers the caret {:?}",
            p.rect,
            req.anchor,
        );
    }
}
