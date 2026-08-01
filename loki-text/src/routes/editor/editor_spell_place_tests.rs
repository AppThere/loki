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

// ── The scroll-drift check, headless half ────────────────────────────────────
//
// The sitting answers "does the menu land on the word at every scroll offset".
// That question has two halves and only one of them needs a screen:
//
//   1. **Does the platform's `client_*` track the word as the container
//      scrolls?** A property of Blitz's event dispatch. Not testable here.
//   2. **Does our code introduce a scroll term?** Entirely ours, and the half
//      that has been wrong twice (r42, r43). Tested below.
//
// Separating them is the point. If the sitting shows drift and these pass, the
// defect is in (1) and no amount of re-reading our arithmetic will find it.

use loki_renderer::TileContext;

use super::spell_menu_anchor;

/// A right-click on the same word, reported by the platform at two different
/// container scroll offsets: the word has moved up the window by `scrolled_by`,
/// so the platform reports a smaller `client_y` — and the tile-local point is
/// **unchanged**, because the click is at the same place within the same page.
fn click_on_the_same_word(client_y: f32, tile_y: f32) -> TileContext {
    TileContext {
        page_index: 3,
        x_pt: 120.0,
        y_pt: tile_y,
        client_x: 400.0,
        client_y,
    }
}

/// **The anchor is the window coordinate, never the tile-local one.**
///
/// Both pairs come off the same event and both are plausible; picking the wrong
/// one compiles and lands the menu at an offset that looks deliberate. This
/// stack also swaps the DOM's `client_*`/`page_*` senses, so the field names
/// cannot be trusted to disambiguate — which is why the choice is a function
/// with a test rather than two field reads at the call site.
#[test]
fn the_anchor_is_the_window_coordinate_and_not_the_tile_local_one() {
    let ctx = click_on_the_same_word(300.0, 87.5);
    assert_eq!(
        spell_menu_anchor(&ctx),
        (400.0, 300.0),
        "the anchor must be `client_*`; `x_pt`/`y_pt` are layout points inside \
         the tile and would place the menu near the top-left of the page",
    );
}

/// **The drift property itself, as far as it can be established headlessly.**
///
/// Scrolling the editor moves the word up the window, so the platform reports a
/// smaller `client_y` for a click on it. The anchor must follow that figure
/// *exactly* — any scroll compensation in our code would show up as a difference
/// between the reported delta and the anchor delta, and that difference is
/// precisely the "offset grows with scroll" reading in the procedure docs.
///
/// The tile-local `y_pt` is deliberately held constant across the sweep: the
/// click is on the same word in the same page, so a correct implementation must
/// ignore it here, and an implementation that mixed the two pairs would produce
/// an anchor that does *not* move with the window figure.
#[test]
fn the_anchor_tracks_the_window_figure_with_no_scroll_term_of_our_own() {
    let baseline = spell_menu_anchor(&click_on_the_same_word(600.0, 87.5));
    for scrolled_by in [0.0_f32, 1.0, 40.0, 41.0, 240.0, 599.0] {
        let ctx = click_on_the_same_word(600.0 - scrolled_by, 87.5);
        let (_, anchor_y) = spell_menu_anchor(&ctx);
        assert!(
            (baseline.1 - anchor_y - scrolled_by).abs() < 0.001,
            "scrolled {scrolled_by}px: the anchor moved {}px, not {scrolled_by}px \
             — a difference here is a scroll term in our own path, which on a \
             screen reads as the menu drifting further from the word the further \
             the document is scrolled",
            baseline.1 - anchor_y,
        );
    }
}

/// The polarity that stops the two tests above being satisfied by a constant:
/// the anchor must also be *sensitive* to the window figure. Without this,
/// `spell_menu_anchor` returning `(400.0, 300.0)` unconditionally passes the
/// first test and — being constant — the second one too.
#[test]
fn a_click_somewhere_else_anchors_somewhere_else() {
    assert_ne!(
        spell_menu_anchor(&click_on_the_same_word(300.0, 87.5)),
        spell_menu_anchor(&click_on_the_same_word(301.0, 87.5)),
    );
}

/// **41 is not a magic number and must not reappear.** The pre-migration defect
/// put the menu one tab-bar below the click, and the shape that produced it was
/// a constant offset applied somewhere in this path. Asserted at the whole
/// placement rather than at the anchor, so it also covers `spell_menu_placement`
/// growing a compensation later.
#[test]
fn no_fixed_chrome_offset_survives_anywhere_in_the_path() {
    let ctx = click_on_the_same_word(300.0, 87.5);
    let (ax, ay) = spell_menu_anchor(&ctx);
    let req = spell_menu_placement(ax, ay);
    assert_eq!(
        (req.anchor.x, req.anchor.y),
        (ctx.client_x, ctx.client_y),
        "a constant between the click and the placement request is the ~41px \
         defect returning; on a screen it reads as the same offset at every \
         scroll position",
    );
}
