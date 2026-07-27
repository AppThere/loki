// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The three interaction decisions, asserted as the failures a user would hit.

use super::super::geometry::{place, Align, PlacementRequest, Rect, Side};
use std::sync::Mutex;

use super::{
    anchor_is_anchorable, focus_after_dismiss, on_anchor_change, repositions, route_key,
    AnchorResponse, DismissCause, FocusTarget, Key, KeyAction, Role,
};

/// Serialises every test that reads **or moves** the process-wide reposition
/// counter.
///
/// `cargo test` runs tests in parallel, so two tests sharing one global would
/// interleave: reset-then-assert-zero can observe another test's increment, and
/// the failure appears as an intermittent CI red with no local reproduction.
/// Holding a lock **and comparing deltas rather than absolutes** removes both
/// halves — no reset is needed, so nothing is destroyed for a concurrent reader.
///
/// **Writers must hold it too**, which the first draft missed: a delta is only a
/// delta if nothing else increments in between, so a test that merely *causes* a
/// reposition races the two that measure one. Latent while every such test did a
/// single reposition; the boundary sweep below does hundreds, which would have
/// turned it into a regular flake.
static COUNTER_LOCK: Mutex<()> = Mutex::new(());

/// A tall viewport, so `a_resize_that_leaves_the_anchor_still_re_places` can
/// shorten it and change the answer.
const VIEWPORT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 900.0,
    height: 1400.0,
};

fn req_at(anchor: Rect, viewport: Rect) -> PlacementRequest {
    PlacementRequest {
        anchor,
        width: 300.0,
        height: 320.0,
        viewport,
        preferred: Side::Below,
        align: Align::Start,
        gap: 4.0,
        margin: 8.0,
    }
}

/// **The failure that makes `Role` necessary:** arrows stolen from a control
/// inside a panel.
///
/// A colour picker's slider and a zoom field both need their own arrow keys. If
/// the popover claims them for item navigation, the control cannot be used from
/// the keyboard at all.
#[test]
fn a_panel_never_steals_arrows_from_its_controls() {
    for key in [Key::Down, Key::Up, Key::Home, Key::End] {
        assert_eq!(
            route_key(Role::Panel, key),
            KeyAction::PassThrough,
            "{key:?} was claimed by the panel; the focused control needs it",
        );
    }
}

/// **The other half of the same failure:** Tab dismissing a panel mid-edit.
///
/// A picker with three controls must let Tab move between them. Closing instead
/// loses whatever was being adjusted.
#[test]
fn a_panel_cycles_on_tab_rather_than_closing() {
    assert_eq!(
        route_key(Role::Panel, Key::Tab),
        KeyAction::FocusNextControl
    );
    assert_eq!(
        route_key(Role::Panel, Key::ShiftTab),
        KeyAction::FocusPrevControl,
    );
}

/// A menu is the mirror image: arrows navigate, Tab leaves.
#[test]
fn a_menu_navigates_with_arrows_and_leaves_on_tab() {
    assert_eq!(route_key(Role::Menu, Key::Down), KeyAction::Next);
    assert_eq!(route_key(Role::Menu, Key::Up), KeyAction::Prev);
    assert_eq!(route_key(Role::Menu, Key::Home), KeyAction::First);
    assert_eq!(route_key(Role::Menu, Key::End), KeyAction::Last);
    assert_eq!(
        route_key(Role::Menu, Key::Tab),
        KeyAction::DismissAndAdvance,
    );
    assert_eq!(
        route_key(Role::Menu, Key::Char('r')),
        KeyAction::Typeahead('r')
    );
}

/// Escape is the one key with no per-role behaviour, and it must close both —
/// a panel that could only be closed by clicking away would trap a keyboard
/// user inside it.
#[test]
fn escape_closes_both_roles() {
    for role in [Role::Menu, Role::Panel] {
        assert_eq!(route_key(role, Key::Escape), KeyAction::Dismiss, "{role:?}");
    }
}

/// The trap follows the role rather than being set beside it, so "a menu that
/// traps focus" is a state this API cannot reach.
#[test]
fn the_focus_trap_is_derived_from_the_role() {
    assert!(!Role::Menu.traps_focus(), "Tab is a menu's exit");
    assert!(Role::Panel.traps_focus(), "Tab cycles within a panel");
}

/// **The most common accessibility defect in this component class:** Escape
/// closes the popover and focus lands nowhere, ejecting a keyboard user to the
/// top of the page.
#[test]
fn dismissal_returns_focus_to_the_control_that_opened_it() {
    assert_eq!(
        focus_after_dismiss(DismissCause::Escape),
        FocusTarget::Anchor,
        "Escape must put the user back where they were",
    );
    assert_eq!(
        focus_after_dismiss(DismissCause::Activated),
        FocusTarget::Anchor
    );
    assert_eq!(
        focus_after_dismiss(DismissCause::AnchorScrolledAway),
        FocusTarget::Anchor,
    );
}

/// The two causes that already moved focus deliberately must not fight it back.
///
/// Returning focus to the anchor after an outside click would yank it away from
/// whatever the user just clicked; returning it after Tab would make Tab a
/// no-op, which reads as a stuck keyboard.
#[test]
fn causes_that_moved_focus_deliberately_are_left_alone() {
    assert_eq!(
        focus_after_dismiss(DismissCause::OutsideClick),
        FocusTarget::Unchanged,
    );
    assert_eq!(
        focus_after_dismiss(DismissCause::TabOut),
        FocusTarget::PastAnchor,
    );
}

/// **The case T4.2 walks into:** a menu anchored to an entry inside the very
/// list the user scrolls to reach entries.
///
/// A flat dismiss-on-scroll rule closes the menu on the first trackpad nudge.
#[test]
fn scrolling_the_anchors_own_list_repositions_rather_than_closing() {
    let _guard = COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = req_at(Rect::new(100.0, 300.0, 200.0, 24.0), VIEWPORT);
    let after = req_at(Rect::new(100.0, 280.0, 200.0, 24.0), VIEWPORT);
    assert!(
        matches!(
            on_anchor_change(before, after, true),
            AnchorResponse::Reposition(_)
        ),
        "a nudge that leaves the entry visible must move the menu, not close it",
    );
}

/// Once the anchor is gone there is nothing to anchor to.
#[test]
fn scrolling_the_anchor_out_of_view_dismisses() {
    let r = req_at(Rect::new(100.0, 300.0, 200.0, 24.0), VIEWPORT);
    assert_eq!(on_anchor_change(r, r, false), AnchorResponse::Dismiss);
}

/// **The band neither module owned.** `place` clamps an overlay into the
/// viewport whatever the anchor does; `on_anchor_change` dismissed on a caller's
/// `bool`. A caller that answered "still visible" for an anchor scrolled off the
/// screen therefore kept a popover alive, pinned to a viewport edge, pointing at
/// nothing — and no test anywhere would have said so, because each module was
/// individually right.
///
/// The caller's `bool` no longer covers the viewport: it answers only for the
/// things geometry cannot see.
#[test]
fn an_anchor_off_screen_dismisses_even_when_its_container_says_otherwise() {
    let _guard = COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gone = req_at(Rect::new(100.0, -600.0, 200.0, 24.0), VIEWPORT);
    assert!(
        !anchor_is_anchorable(gone.anchor, gone.viewport),
        "precondition: the fixture's anchor must be wholly off-screen",
    );
    assert_eq!(
        on_anchor_change(gone, gone, true),
        AnchorResponse::Dismiss,
        "the caller said the anchor was still in its container, and it is off \
         the screen — the viewport half of that judgement is not the caller's",
    );
}

/// A caret is a **zero-width** rect, so a strict intersection test would report
/// every caret sitting exactly on a viewport edge as gone and dismiss the
/// spelling menu on the left margin. Touching counts.
#[test]
fn a_caret_touching_the_viewport_edge_is_still_anchorable() {
    let vp = Rect::new(0.0, 34.0, 900.0, 742.0);
    for caret in [
        Rect::new(0.0, 300.0, 0.0, 18.0),   // On the left edge.
        Rect::new(900.0, 300.0, 0.0, 18.0), // On the right edge.
        Rect::new(100.0, 16.0, 0.0, 18.0),  // Bottom exactly on the top inset.
        Rect::new(100.0, 776.0, 0.0, 18.0), // Top exactly on the bottom edge.
    ] {
        assert!(
            anchor_is_anchorable(caret, vp),
            "caret {caret:?} touching viewport {vp:?} was called un-anchorable",
        );
    }
}

/// **The two modules agree on where the boundary is**, swept rather than argued.
///
/// The property: for every anchor, *either* it is anchorable — and then `place`
/// yields an overlay inside the viewport that does not cover it, so keeping the
/// popover open is meaningful — *or* it is not, and `on_anchor_change` dismisses.
/// There is no third outcome, which is exactly what "no unowned band" means.
///
/// Swept across the edges and well past them, because the case that existed was
/// a *partial* overlap: it is what every scroll passes through, and it is the
/// region hand-written fixtures skip on their way from inside to outside.
#[test]
fn the_two_modules_agree_on_the_anchor_visibility_boundary() {
    let _guard = COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let vp = Rect::new(0.0, 34.0, 900.0, 742.0);
    let mut anchorable = 0_u32;
    let mut dismissed = 0_u32;
    for ax in [-300.0_f32, -1.0, 0.0, 100.0, 899.0, 900.0, 901.0, 1200.0] {
        for ay in [-300.0_f32, -20.0, 15.0, 34.0, 400.0, 775.0, 776.0, 1000.0] {
            for (w, h) in [(0.0_f32, 18.0_f32), (200.0, 24.0), (2.0, 0.0)] {
                let req = req_at(Rect::new(ax, ay, w, h), vp);
                if anchor_is_anchorable(req.anchor, req.viewport) {
                    anchorable += 1;
                    let p = place(req);
                    assert!(
                        p.rect.is_inside(vp),
                        "anchor {:?} is anchorable, so the overlay must be on \
                         screen; got {:?}",
                        req.anchor,
                        p.rect,
                    );
                    assert!(
                        !p.rect.covers_vertically_open(req.anchor),
                        "overlay {:?} covers the anchor {:?} it is kept open for",
                        p.rect,
                        req.anchor,
                    );
                } else {
                    dismissed += 1;
                    assert_eq!(
                        on_anchor_change(req, req, true),
                        AnchorResponse::Dismiss,
                        "anchor {:?} is outside viewport {vp:?} and was not \
                         dismissed",
                        req.anchor,
                    );
                }
            }
        }
    }
    assert!(
        anchorable > 0 && dismissed > 0,
        "the sweep must reach both sides of the boundary to say anything: \
         {anchorable} anchorable, {dismissed} dismissed",
    );
}

/// The direction a flat rule gets wrong the other way: an unrelated pane
/// scrolling must not close a menu the user is reading. Falls out of the anchor
/// rect being unchanged — no separate case needed.
#[test]
fn an_unrelated_scroll_leaves_the_popover_alone() {
    let r = req_at(Rect::new(100.0, 300.0, 200.0, 24.0), VIEWPORT);
    assert_eq!(on_anchor_change(r, r, true), AnchorResponse::Ignore);
}

/// **The gap a container-visibility predicate would have left.** An entry can sit
/// unmoved and fully visible inside its list while the *list* scrolls in the
/// page — so the anchor's viewport rect changes even though nothing about its
/// container did, and the flip decision it was placed with is stale.
///
/// The assertion is on the outcome a reader would see: after repositioning, the
/// popover is inside the viewport. A stale placement is one that is not.
#[test]
fn a_container_scrolling_within_the_page_re_places_against_the_viewport() {
    let _guard = COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // A 700-tall viewport, not the module's tall one: the point is that moving
    // the anchor down it exhausts the room below, which a 1400-tall viewport
    // would not do — an earlier draft used it and the test passed while
    // asserting nothing about a flip.
    let screen = Rect::new(0.0, 0.0, 900.0, 700.0);
    // Placed with room below: anchor high in the viewport, opens downward.
    let before = req_at(Rect::new(100.0, 100.0, 200.0, 24.0), screen);
    let placed = place(before);
    assert_eq!(placed.side, Side::Below, "precondition: it opened downward");
    assert!(
        !placed.flipped && !placed.clamped,
        "precondition: the original placement must be the unconstrained one, or \
         the staleness under test is not what is being observed",
    );

    // The list scrolls down the page. The entry has not moved inside its list,
    // but it is now near the bottom of the viewport.
    let after = req_at(Rect::new(100.0, 660.0, 200.0, 24.0), screen);
    let room_below = after.viewport.bottom() - after.anchor.bottom() - after.gap - after.margin;
    assert!(
        room_below < after.height,
        "fixture no longer creates the condition: after the scroll there is \
         still {room_below}px below for a {}px overlay, so nothing would go \
         stale and this test asserts nothing",
        after.height,
    );
    let AnchorResponse::Reposition(p) = on_anchor_change(before, after, true) else {
        panic!("expected a reposition");
    };
    assert!(
        p.rect.is_inside(after.viewport),
        "re-placed overlay {:?} hangs outside the viewport — offsetting by the \
         scroll delta would have preserved the stale downward flip",
        p.rect,
    );
    assert_eq!(p.side, Side::Above, "there is no longer room below");
}

/// **Window resize is the same class and must route the same way.** The anchor
/// has not moved at all; the bounds it was placed against have.
///
/// Given its own path, resize and scroll drift apart — and resize is the one
/// that gets forgotten, because nothing moved.
#[test]
fn a_resize_that_leaves_the_anchor_still_re_places() {
    let _guard = COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = req_at(Rect::new(100.0, 600.0, 200.0, 24.0), VIEWPORT);
    // The window shortens: the anchor is unchanged, the room below is not.
    let after = req_at(
        Rect::new(100.0, 600.0, 200.0, 24.0),
        Rect::new(0.0, 0.0, 900.0, 700.0),
    );
    let r = on_anchor_change(before, after, true);
    assert!(
        matches!(r, AnchorResponse::Reposition(_)),
        "a resize with an unmoved anchor must still re-place; got {r:?}",
    );
    let AnchorResponse::Reposition(p) = r else {
        unreachable!()
    };
    assert!(p.rect.is_inside(after.viewport));
}

/// **A menu must handle every key itself.**
///
/// A menu has no inner control to pass a key to, so `PassThrough` there is a
/// dead key — it presents as an unresponsive keyboard, which is hard to tell
/// from a focus problem and sends you looking in the wrong place.
///
/// This replaced a test that called `route_key` for every key and discarded the
/// result, with a `keys.len() == 9` line claiming to catch a new `Key` variant.
/// It could not: adding a variant does not change an array nobody updated. A
/// test that cannot fail for the reason it states is worse than no test, because
/// it occupies the space where a real one would go.
#[test]
fn a_menu_handles_every_key_itself() {
    for key in [
        Key::Down,
        Key::Up,
        Key::Home,
        Key::End,
        Key::Activate,
        Key::Escape,
        Key::Tab,
        Key::ShiftTab,
        Key::Char('a'),
    ] {
        assert_ne!(
            route_key(Role::Menu, key),
            KeyAction::PassThrough,
            "{key:?} falls through in a menu, where there is nothing to catch it",
        );
    }
}

/// A panel passes through everything it does not itself own, so a control can
/// always be driven from the keyboard. Only Tab, Shift+Tab and Escape are the
/// popover's.
#[test]
fn a_panel_claims_only_tab_and_escape() {
    for key in [
        Key::Down,
        Key::Up,
        Key::Home,
        Key::End,
        Key::Activate,
        Key::Char('a'),
    ] {
        assert_eq!(
            route_key(Role::Panel, key),
            KeyAction::PassThrough,
            "{key:?} was claimed by the panel rather than left to its controls",
        );
    }
    assert_ne!(route_key(Role::Panel, Key::Tab), KeyAction::PassThrough);
    assert_ne!(route_key(Role::Panel, Key::Escape), KeyAction::PassThrough);
}

/// **The jitter loop, made observable.** The anchor comparison is exact float
/// equality, so a layout that returns a sub-pixel-different rect for an unmoved
/// anchor would re-place every frame.
///
/// Exact is still the right comparison — an epsilon buys a jitter loop off at the
/// price of staleness, which is the defect this module exists to prevent. What
/// was missing was a way to *see* a loop, since it presents as a frame-rate
/// symptom and sends you looking at rendering.
///
/// Asserted the way a consumer should assert it at runtime: nothing moved, so
/// the count must not advance.
#[test]
fn idle_frames_perform_no_repositions() {
    let guard = COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let r = req_at(Rect::new(100.0, 300.0, 200.0, 24.0), VIEWPORT);
    let before = repositions();
    for _ in 0..120 {
        assert_eq!(on_anchor_change(r, r, true), AnchorResponse::Ignore);
    }
    let moved = repositions() - before;
    drop(guard);
    assert_eq!(
        moved, 0,
        "an unmoved anchor re-placed {moved} times across 120 idle frames — the \
         exact-equality comparison is seeing jitter from layout",
    );
}

/// The counter must actually count, or the guard above passes for the wrong
/// reason — a counter that never increments reports quiet as easily as a loop.
#[test]
fn a_real_move_advances_the_reposition_counter() {
    let guard = COUNTER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let before = req_at(Rect::new(100.0, 300.0, 200.0, 24.0), VIEWPORT);
    let after = req_at(Rect::new(100.0, 280.0, 200.0, 24.0), VIEWPORT);
    let start = repositions();
    let _ = on_anchor_change(before, after, true);
    let moved = repositions() - start;
    drop(guard);
    assert_eq!(
        moved, 1,
        "a real move must advance the counter exactly once"
    );
}

/// **The cause root hosting created.** Rendered beside its trigger, a popup died
/// with its subtree; hosted at the root it does not, so a navigation leaves a
/// menu outliving the screen it belongs to.
///
/// Focus is left alone: there is no anchor to return to, and a navigation has
/// already placed focus on whatever replaced the screen.
#[test]
fn an_unmounted_anchor_dismisses_without_moving_focus() {
    assert_eq!(
        focus_after_dismiss(DismissCause::AnchorUnmounted),
        FocusTarget::Unchanged,
        "an unmount has no anchor to restore to, and the navigation that caused \
         it has already placed focus",
    );
}

/// **What `route_key` cannot say:** Escape closing a menu must not also reach the
/// editor beneath, or one keypress has two effects and the second is invisible
/// until someone loses an edit.
#[test]
fn everything_the_popover_handles_is_also_consumed() {
    for role in [Role::Menu, Role::Panel] {
        for key in [
            Key::Down,
            Key::Up,
            Key::Home,
            Key::End,
            Key::Activate,
            Key::Escape,
            Key::Tab,
            Key::ShiftTab,
            Key::Char('a'),
        ] {
            let action = route_key(role, key);
            assert_eq!(
                action.consumes(),
                action != KeyAction::PassThrough,
                "{role:?} / {key:?} -> {action:?}: handled keys must stop \
                 propagation and passed-through keys must not",
            );
        }
    }
}

/// The half that a blanket "consume everything" would break: a panel's own
/// controls must still see their keys.
#[test]
fn a_passed_through_key_is_not_consumed() {
    assert!(!KeyAction::PassThrough.consumes());
    assert!(!route_key(Role::Panel, Key::Down).consumes());
    assert!(route_key(Role::Panel, Key::Escape).consumes());
}
