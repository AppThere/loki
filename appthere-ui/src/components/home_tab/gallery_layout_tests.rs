// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! T4.4's layout decision, asserted at the size classes a development machine
//! never reaches.
//!
//! **This is the same problem T4.0 named for the pointer**, one axis over: a
//! large display is always `Expanded`, so the capped form runs in no session
//! anybody has and a screen check would report Phase 4 clear on the strength of
//! a branch that never ran. `LOKI_DEVICE_PROFILE`'s window override forces it on
//! a screen; these force it without one.

use super::{
    compact_max_height, gallery_layout, CARD_GAP_PX, CARD_HEIGHT_PX, CARD_WIDTH_PX,
    COMPACT_VISIBLE_ROWS,
};
use crate::responsive::Breakpoint;
use crate::tokens::spacing::TOUCH_MIN;

/// **The narrow form caps and scrolls.** The gallery shares one column with the
/// recent documents list there; an uncapped grid of every template would push
/// the documents — the reason most sessions open Home at all — off the screen.
#[test]
fn the_narrow_classes_cap_the_gallery_height() {
    for bp in [Breakpoint::Compact, Breakpoint::Medium] {
        assert_eq!(
            gallery_layout(bp).max_height_px,
            Some(compact_max_height()),
            "{bp:?} must cap the gallery, or the recent list is pushed off screen",
        );
    }
}

/// **The polarity (L08-045).** Wide, the gallery owns the full left column, so
/// its natural height is the right height — an internal scrollbar beside an
/// unused one is noise. Without this, `gallery_layout` returning `Some(..)`
/// unconditionally passes the test above and the wide form is never checked.
#[test]
fn the_wide_class_does_not_cap_it() {
    assert_eq!(gallery_layout(Breakpoint::Expanded).max_height_px, None);
}

/// The cap is **two rows plus the gap between them**, derived from the card
/// rather than written down. A cap that did not follow a padding change would
/// clip the second row's labels, which is precisely the "shows one row, reads as
/// the whole set" failure the two-row rule exists to prevent.
#[test]
fn the_cap_is_two_rows_and_one_gap() {
    let expected = 2.0 * CARD_HEIGHT_PX + CARD_GAP_PX;
    assert!(
        (compact_max_height() - expected).abs() < f32::EPSILON,
        "cap is {}, not two rows ({CARD_HEIGHT_PX}) plus one gap ({CARD_GAP_PX})",
        compact_max_height(),
    );
    assert!(
        (COMPACT_VISIBLE_ROWS - 2.0).abs() < f32::EPSILON,
        "the row count moved; the cap's derivation must move with it",
    );
}

/// **The cap must show more than one row.** If the derivation ever produced a
/// height under two cards, the gallery would show one row and read as the whole
/// set — the failure the two-row choice exists to prevent, arriving through the
/// arithmetic rather than through the constant.
#[test]
fn the_cap_is_strictly_taller_than_one_row() {
    assert!(
        compact_max_height() > CARD_HEIGHT_PX + CARD_GAP_PX,
        "a cap of {} shows one row and a sliver, which reads as the whole set",
        compact_max_height(),
    );
}

/// Cards stay touch-legal. A wrapping grid is free to make cards smaller than a
/// row-scroller had to; this is the floor that stops it.
#[test]
fn a_card_is_at_least_a_touch_target_in_both_axes() {
    assert!(
        CARD_WIDTH_PX >= TOUCH_MIN,
        "card width {CARD_WIDTH_PX} is under the {TOUCH_MIN}px minimum",
    );
    assert!(
        CARD_HEIGHT_PX >= TOUCH_MIN,
        "card height {CARD_HEIGHT_PX} is under the {TOUCH_MIN}px minimum",
    );
}
