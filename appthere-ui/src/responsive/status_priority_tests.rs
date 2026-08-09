// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the status-bar priority-drop engine (Spec 08 T7.1).

use super::{estimate_label_px, resolve_status_fit, StatusItem};
use crate::tokens::layout::{RIBBON_COLLAPSE_HYSTERESIS_PX, RIBBON_OVERFLOW_BUTTON_PX};

fn item(priority: u8, width_px: f32) -> StatusItem {
    StatusItem {
        priority,
        width_px,
        retained: false,
    }
}

fn kept(width_px: f32) -> StatusItem {
    StatusItem {
        priority: 0,
        width_px,
        retained: true,
    }
}

/// The bar as the status bar builds it: two retained items and three droppable
/// ones at descending priority, 100 px each.
fn bar() -> Vec<StatusItem> {
    vec![
        kept(100.0),    // 0 page indicator
        item(1, 100.0), // 1 word count      (drops first)
        item(3, 100.0), // 2 view mode
        item(2, 100.0), // 3 language        (drops second)
        kept(100.0),    // 4 zoom
    ]
}

/// Everything fits: nothing drops, no overflow trigger, no floor.
#[test]
fn a_wide_bar_shows_everything() {
    let fit = resolve_status_fit(&bar(), 1000.0, 0);
    assert!(fit.shown.iter().all(|&s| s), "{:?}", fit.shown);
    assert_eq!(fit.dropped, 0);
    assert!(!fit.overflow);
    assert!(!fit.scroll);
}

/// **Items drop in ascending priority, not in layout order.** The lowest
/// priority goes first even though it is not the leftmost or the rightmost
/// droppable item — a drop order that fell back to position would pick a
/// different one.
#[test]
fn the_lowest_priority_item_drops_first() {
    // Room for four items plus the overflow trigger, so exactly one must go.
    let width = 400.0 + RIBBON_OVERFLOW_BUTTON_PX;
    let fit = resolve_status_fit(&bar(), width, 0);
    assert_eq!(fit.dropped, 1, "expected exactly one drop: {fit:?}");
    assert!(!fit.shown[1], "the priority-1 item should have dropped");
    assert!(
        fit.shown[2] && fit.shown[3],
        "a higher-priority item dropped"
    );
    assert!(fit.overflow);

    // Narrower still: the priority-2 item joins it, the priority-3 one does not.
    let fit = resolve_status_fit(&bar(), 300.0 + RIBBON_OVERFLOW_BUTTON_PX, 0);
    assert_eq!(fit.dropped, 2, "{fit:?}");
    assert!(!fit.shown[1] && !fit.shown[3]);
    assert!(
        fit.shown[2],
        "the highest-priority droppable item dropped early"
    );
}

/// **The retention set survives any width, and the floor is reported rather than
/// hidden.** This is the assertion the whole `retained` flag exists for: at a
/// width that fits neither retained item, both are still shown and `scroll` says
/// the bar is narrower than its minimum.
#[test]
fn retained_items_never_drop_however_narrow_it_gets() {
    for width in [200.0f32, 100.0, 40.0, 1.0] {
        let fit = resolve_status_fit(&bar(), width, 0);
        assert!(
            fit.shown[0] && fit.shown[4],
            "a retained item dropped at {width}px: {fit:?}"
        );
        assert!(
            fit.dropped <= 3,
            "dropped more than the three droppable items at {width}px: {fit:?}"
        );
    }
    let fit = resolve_status_fit(&bar(), 40.0, 0);
    assert_eq!(fit.dropped, 3, "everything droppable should be gone");
    assert!(fit.scroll, "the floor was not reported: {fit:?}");
}

/// The inverse of the floor: a width that *does* fit the retained pair plus the
/// trigger reports no floor. Without this, `scroll` could be hardwired true.
#[test]
fn the_floor_is_not_reported_when_the_minimum_fits() {
    let fit = resolve_status_fit(&bar(), 200.0 + RIBBON_OVERFLOW_BUTTON_PX + 1.0, 0);
    assert_eq!(fit.dropped, 3);
    assert!(
        !fit.scroll,
        "reported a floor at a width that fits: {fit:?}"
    );
}

/// **Hysteresis: a width that just lost an item does not immediately regain it.**
/// Re-widening by less than the band leaves the drop in place; clearing the band
/// restores it. A resolver without hysteresis flips on every pixel of a drag.
#[test]
fn restoring_an_item_needs_the_hysteresis_band() {
    let items = bar();
    // Settled at one drop.
    let narrow = 400.0 + RIBBON_OVERFLOW_BUTTON_PX;
    let settled = resolve_status_fit(&items, narrow, 0);
    assert_eq!(settled.dropped, 1);

    // Just enough for all five (500) but not 500 + band: the drop holds.
    let inside_band = 500.0 + RIBBON_COLLAPSE_HYSTERESIS_PX - 1.0;
    let held = resolve_status_fit(&items, inside_band, settled.dropped);
    assert_eq!(
        held.dropped, 1,
        "restored inside the hysteresis band: {held:?}"
    );

    // Past the band: restored.
    let past_band = 500.0 + RIBBON_COLLAPSE_HYSTERESIS_PX;
    let restored = resolve_status_fit(&items, past_band, settled.dropped);
    assert_eq!(
        restored.dropped, 0,
        "did not restore past the band: {restored:?}"
    );
}

/// Idempotent at a fixed width — resolving from its own result must not move.
/// A cascade that drifted would oscillate under the effect that feeds it back.
#[test]
fn resolution_is_idempotent_at_a_fixed_width() {
    let items = bar();
    for width in [1000.0f32, 620.0, 480.0, 330.0, 150.0] {
        let once = resolve_status_fit(&items, width, 0);
        let twice = resolve_status_fit(&items, width, once.dropped);
        assert_eq!(once, twice, "not idempotent at {width}px");
    }
}

/// An unmeasured width holds the previous state rather than treating "not yet
/// measured" as "no room", which would empty the bar on the first frame.
#[test]
fn an_unmeasured_width_changes_nothing() {
    let items = bar();
    for prev in [0usize, 2] {
        let fit = resolve_status_fit(&items, 0.0, prev);
        assert_eq!(fit.dropped, prev, "an unmeasured width moved the state");
    }
    assert!(
        !resolve_status_fit(&items, 0.0, 0).scroll,
        "an unmeasured width reported the floor"
    );
}

/// **The overflow trigger costs width, and the engine pays for it.**
///
/// The discriminating width is one that fits four items but *not* four items
/// plus the trigger: `400 < available < 400 + RIBBON_OVERFLOW_BUTTON_PX`. An
/// engine that charged nothing for the trigger stops after one drop there; one
/// that charges for it must drop a second.
///
/// The first version of this test asserted `dropped == 0` at exactly 500 px and
/// **survived** deleting the trigger's width from the sum — at that width the
/// two engines agree, so it was measuring nothing. Kept as a note because the
/// replacement looks arbitrary without it.
#[test]
fn the_overflow_trigger_is_counted_against_the_width() {
    let btn = RIBBON_OVERFLOW_BUTTON_PX;
    let available = 400.0 + btn / 2.0;
    assert!(
        available > 400.0 && available < 400.0 + btn,
        "the fixture width no longer sits in the discriminating band"
    );

    let fit = resolve_status_fit(&bar(), available, 0);
    assert_eq!(
        fit.dropped, 2,
        "four items fit but four-plus-the-trigger do not, so a second item must \
         drop — one drop means the trigger was charged nothing: {fit:?}"
    );
    assert!(fit.overflow);

    // And the inverse: at a width that fits everything, nothing drops and no
    // trigger is charged — so the assertion above is about the trigger and not
    // a blanket "always drops two".
    let roomy = resolve_status_fit(&bar(), 500.0, 0);
    assert_eq!(roomy.dropped, 0, "dropped an item that fitted: {roomy:?}");
    assert!(!roomy.overflow);
}

/// **Wide scripts are not counted as narrow ones.** A label of CJK characters
/// paints about twice the width of the same number of Latin letters; declaring
/// it narrow under-estimates, and under-estimating is the direction that
/// overflows the bar instead of dropping an item.
#[test]
fn a_wide_script_label_declares_more_width_than_a_latin_one() {
    let latin = estimate_label_px("English (US)", 12.0);
    let cjk = estimate_label_px("日本語", 12.0);
    // Three wide chars against twelve narrow ones: the ratio, not the sign, is
    // what this checks — 3 em vs 12 * 0.55 = 6.6 em.
    assert!(cjk > 0.0 && latin > 0.0);
    assert!(
        (cjk / 3.0) > (latin / 12.0) * 1.5,
        "per-character width of a wide script ({}) is not meaningfully larger \
         than a narrow one ({})",
        cjk / 3.0,
        latin / 12.0
    );
    // And the estimate scales with the font size, which is the other half of
    // what makes it a width rather than a character count.
    assert!(estimate_label_px("abc", 24.0) > estimate_label_px("abc", 12.0));
    assert_eq!(estimate_label_px("", 12.0), 0.0);
}

/// A bar with nothing droppable resolves without dropping anything and without
/// looping — the degenerate case the drop-order construction has to survive.
#[test]
fn a_bar_of_only_retained_items_terminates() {
    let items = vec![kept(100.0), kept(100.0)];
    let fit = resolve_status_fit(&items, 10.0, 0);
    assert_eq!(fit.dropped, 0);
    assert!(
        !fit.overflow,
        "an overflow trigger with nothing to put in it"
    );
    assert!(fit.scroll, "the floor was not reported");
    assert!(fit.shown.iter().all(|&s| s));
}
