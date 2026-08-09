// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The Recent Documents menu's two decisions: which document it belongs to, and
//! where it goes.
//!
//! The component itself needs a Dioxus runtime and is not covered here. What is
//! covered is everything the component *consults* — which is where the defect
//! this task removes actually lived.

use super::{key_for_path, recent_menu_placement};
use crate::components::popover::{
    on_anchor_identity, place, IdentityCheck, Rect, Side, MIN_ANCHORED_MENU_PX,
};

/// A window with room in every direction.
const VIEWPORT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1280.0,
    height: 800.0,
};

/// A ⋮ button at the right-hand end of a row.
fn trigger_at(y: f32) -> Rect {
    Rect {
        x: 380.0,
        y,
        width: 44.0,
        height: 44.0,
    }
}

fn placed(anchor: Rect) -> crate::components::popover::Placement {
    let mut req = recent_menu_placement(anchor);
    req.viewport = VIEWPORT;
    place(req)
}

/// **The defect T4.2 removes, stated as the user's loss.** The list's open-menu
/// state was an index. Recents reorder — opening a document moves it to the top,
/// and removing one shifts everything below it — so a menu opened on the
/// document at position 3 stays attached to *position 3* while a different
/// document moves into it. One of the three actions is **Delete file**.
///
/// The key is derived from the path, so the check catches it.
#[test]
fn a_reordered_list_under_an_open_menu_is_recycled_not_the_same() {
    let opened_for = key_for_path("/docs/quarterly-report.docx");
    // The list reorders; position 3 now holds a different document.
    let under_anchor = Some(key_for_path("/docs/notes.docx"));
    let check = on_anchor_identity(opened_for, under_anchor);
    assert_eq!(
        check,
        IdentityCheck::Recycled,
        "the menu is still pointing at position 3, but position 3 is now a \
         different document — an index-keyed menu would have deleted it",
    );
    assert!(check.must_dismiss());
}

/// The document leaving the list entirely — the other half of the same failure,
/// and the one `Remove from recents` produces on its own neighbour.
#[test]
fn a_document_that_left_the_list_is_gone() {
    let check = on_anchor_identity(key_for_path("/docs/quarterly-report.docx"), None);
    assert_eq!(check, IdentityCheck::Gone);
    assert!(check.must_dismiss());
}

/// **The polarity that makes the two above mean something (L08-045):** the
/// ordinary case must survive. Without this, `must_dismiss()` returning `true`
/// unconditionally passes both — and a menu that closes the instant it opens is
/// a worse defect than the one being fixed.
#[test]
fn an_unchanged_list_keeps_the_menu_open() {
    let key = key_for_path("/docs/quarterly-report.docx");
    let check = on_anchor_identity(key, Some(key));
    assert_eq!(check, IdentityCheck::Same);
    assert!(!check.must_dismiss());
}

/// Identity is derived from content, not position. Two documents at the same
/// index in different renders must not compare equal, and the *same* document
/// must compare equal wherever it has moved to.
#[test]
fn the_key_follows_the_document_and_not_the_slot() {
    let a = "/docs/a.docx";
    let b = "/docs/b.docx";
    assert_eq!(key_for_path(a), key_for_path(a), "same path, same key");
    assert_ne!(
        key_for_path(a),
        key_for_path(b),
        "different paths must differ"
    );
}

/// **The menu opens downward from the ⋮ button and stays inside the window.**
/// `Align::End` because the button sits at the row's right edge — aligning to
/// its start would hang the menu off the list.
#[test]
fn the_menu_opens_below_the_trigger_and_right_aligned_to_it() {
    let anchor = trigger_at(200.0);
    let p = placed(anchor);
    assert_eq!(p.side, Side::Below, "room below at this row");
    let req = recent_menu_placement(anchor);
    assert!(
        (p.rect.y - (anchor.bottom() + req.gap)).abs() < 0.001,
        "menu top {} is not the trigger's bottom plus the {}px gap",
        p.rect.y,
        req.gap,
    );
    assert!(
        (p.rect.right() - anchor.right()).abs() < 0.001,
        "menu right edge {} is not aligned to the trigger's {}",
        p.rect.right(),
        anchor.right(),
    );
    assert!(p.rect.is_inside(VIEWPORT));
}

/// **The bottom-edge case the inline menu could not handle at all.** An inline
/// menu on the last visible row expanded downward inside a container with
/// `overflow-y: auto` and was cut by it. Anchored, it flips.
#[test]
fn a_trigger_near_the_bottom_opens_upward() {
    let anchor = trigger_at(760.0);
    let req = recent_menu_placement(anchor);
    let room_below = VIEWPORT.bottom() - anchor.bottom() - req.gap - req.margin;
    assert!(
        room_below < req.height,
        "fixture no longer creates the condition: {room_below}px below for a \
         {}px menu",
        req.height,
    );
    let p = placed(anchor);
    assert_eq!(p.side, Side::Above);
    assert!(p.rect.is_inside(VIEWPORT), "{:?}", p.rect);
}

/// Containment holds wherever in the list the row sits, not only at the two
/// rows someone thought to check.
#[test]
fn the_menu_stays_on_screen_at_every_row_position() {
    for step in 0..=40 {
        let anchor = trigger_at(step as f32 * 20.0);
        let p = placed(anchor);
        assert!(
            p.rect.is_inside(VIEWPORT),
            "menu {:?} escaped the viewport for a trigger at y={}",
            p.rect,
            anchor.y,
        );
    }
}

/// The menu asks for two whole rows, so a short window scrolls it rather than
/// showing one row that reads as the entire menu. Three actions exist, so
/// "shows one" is a real misreading and not a hypothetical one.
#[test]
fn the_menu_asks_for_two_rows_as_its_anchored_minimum() {
    assert!(
        (recent_menu_placement(trigger_at(200.0)).min_anchored_height - MIN_ANCHORED_MENU_PX).abs()
            < f32::EPSILON,
        "a menu that can shrink below two rows hides that the list continues",
    );
}
