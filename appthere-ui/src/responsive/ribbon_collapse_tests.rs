// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the width-driven ribbon collapse cascade (Spec 04 M3 §7).

use super::{
    estimate_group_metrics, estimate_partial_metrics, group_layout, resolve_cascade, GroupCollapse,
    GroupMetrics,
};
use crate::tokens::layout::{RIBBON_COLLAPSE_HYSTERESIS_PX, RIBBON_OVERFLOW_BUTTON_PX};
use crate::tokens::spacing::{SPACE_1, SPACE_2, TOUCH_MIN};

/// Three groups, priorities low→high left→right, each 100 px full / 50 px
/// condensed. Total full = 300.
fn groups() -> Vec<GroupMetrics> {
    vec![
        GroupMetrics {
            priority: 0,
            full_px: 100.0,
            condensed_px: 50.0,
            partial_px: None,
        },
        GroupMetrics {
            priority: 1,
            full_px: 100.0,
            condensed_px: 50.0,
            partial_px: None,
        },
        GroupMetrics {
            priority: 2,
            full_px: 100.0,
            condensed_px: 50.0,
            partial_px: None,
        },
    ]
}

#[test]
fn everything_full_when_it_all_fits() {
    let c = resolve_cascade(&groups(), 400.0, 0);
    assert_eq!(c.states, vec![GroupCollapse::Full; 3]);
    assert_eq!(c.level, 0);
    assert!(!c.overflow);
    assert!(!c.scroll);
}

#[test]
fn condenses_the_lowest_priority_group_first() {
    // 300 full doesn't fit in 260, but condensing the priority-0 group (−50 →
    // 250) does. Only group 0 condenses; the higher-priority groups stay full.
    let c = resolve_cascade(&groups(), 260.0, 0);
    assert_eq!(
        c.states,
        vec![
            GroupCollapse::Condensed,
            GroupCollapse::Full,
            GroupCollapse::Full,
        ],
    );
    assert!(!c.overflow);
}

#[test]
fn condenses_all_before_overflowing_any() {
    // At 150 px all three must condense (3×50 = 150) but none need overflow.
    let c = resolve_cascade(&groups(), 150.0, 0);
    assert_eq!(c.states, vec![GroupCollapse::Condensed; 3]);
    assert!(!c.overflow);
    assert!(!c.scroll);
}

#[test]
fn overflows_lowest_priority_group_when_condensing_is_not_enough() {
    // 120 px can't hold 3 condensed (150). Overflow the priority-0 group: the
    // strip then holds two condensed (100) + the More button (44) = 144 — still
    // too wide, so a second group overflows: one condensed (50) + More (44) = 94.
    let c = resolve_cascade(&groups(), 120.0, 0);
    assert_eq!(
        c.states,
        vec![
            GroupCollapse::Overflow,
            GroupCollapse::Overflow,
            GroupCollapse::Condensed,
        ],
    );
    assert!(c.overflow);
    assert!(!c.scroll);
}

#[test]
fn scroll_floor_when_even_full_overflow_does_not_fit() {
    // Narrower than just the More button → everything overflows and the strip
    // still can't fit; the horizontal-scroll floor engages.
    let avail = RIBBON_OVERFLOW_BUTTON_PX - 10.0;
    let c = resolve_cascade(&groups(), avail, 0);
    assert_eq!(c.states, vec![GroupCollapse::Overflow; 3]);
    assert_eq!(c.level, 9); // 3 × 3 groups = fully collapsed
    assert!(c.overflow);
    assert!(c.scroll);
}

#[test]
fn unmeasured_width_holds_the_previous_level() {
    // A zero width is "not measured yet" — keep whatever we last resolved.
    let c = resolve_cascade(&groups(), 0.0, 2);
    assert_eq!(c.level, 2);
    // Level 2 = the two lowest-priority groups condensed.
    assert_eq!(
        c.states,
        vec![
            GroupCollapse::Condensed,
            GroupCollapse::Condensed,
            GroupCollapse::Full,
        ],
    );
}

#[test]
fn hysteresis_keeps_a_condensed_group_from_thrashing() {
    // The level 0↔1 boundary sits at 300 px (all three groups full). Collapse
    // happens the instant the strip overflows (avail < 300); re-expansion waits
    // until the full layout clears 300 by the hysteresis band.
    //
    // Sitting just above 300 (inside the dead-band) must hold the collapse.
    let just_over = 301.0;
    assert!(just_over < 300.0 + RIBBON_COLLAPSE_HYSTERESIS_PX);
    let c = resolve_cascade(&groups(), just_over, 1);
    assert_eq!(c.level, 1, "within the dead-band the collapse holds");

    // Well past the band it re-expands to all-full.
    let clear = 300.0 + RIBBON_COLLAPSE_HYSTERESIS_PX + 1.0;
    let c = resolve_cascade(&groups(), clear, 1);
    assert_eq!(c.level, 0);
    assert_eq!(c.states, vec![GroupCollapse::Full; 3]);
}

#[test]
fn resolution_is_idempotent_at_a_fixed_width() {
    // Feeding a resolved level back in at the same width must not move it.
    let avail = 175.0;
    let first = resolve_cascade(&groups(), avail, 0);
    let second = resolve_cascade(&groups(), avail, first.level);
    assert_eq!(first, second);
}

#[test]
fn priority_ties_break_left_to_right() {
    // Two equal-priority groups: the left (lower index) collapses first.
    let equal = vec![
        GroupMetrics {
            priority: 5,
            full_px: 100.0,
            condensed_px: 50.0,
            partial_px: None,
        },
        GroupMetrics {
            priority: 5,
            full_px: 100.0,
            condensed_px: 50.0,
            partial_px: None,
        },
    ];
    let c = resolve_cascade(&equal, 160.0, 0);
    assert_eq!(
        c.states,
        vec![GroupCollapse::Condensed, GroupCollapse::Full],
    );
}

#[test]
fn empty_ribbon_resolves_to_nothing() {
    let c = resolve_cascade(&[], 500.0, 0);
    assert!(c.states.is_empty());
    assert_eq!(c.level, 0);
    assert!(!c.overflow);
    assert!(!c.scroll);
}

#[test]
fn full_layout_keeps_the_label_and_roomy_padding() {
    let lay = group_layout(GroupCollapse::Full, true);
    assert!(lay.rendered);
    assert_eq!(lay.pad_px, SPACE_2);
    assert_eq!(lay.gap_px, 2.0);
    assert!(lay.show_label);
    // A group without a declared label shows none even when full.
    assert!(!group_layout(GroupCollapse::Full, false).show_label);
}

#[test]
fn condensed_layout_drops_the_label_and_tightens() {
    let lay = group_layout(GroupCollapse::Condensed, true);
    assert!(lay.rendered);
    assert_eq!(lay.pad_px, SPACE_1);
    assert_eq!(lay.gap_px, 0.0);
    assert!(!lay.show_label, "the label drops to reclaim width");
    // Compile-time token-scale sanity: condensed padding is tighter than full.
    const { assert!(SPACE_1 < SPACE_2) };
}

#[test]
fn overflow_layout_renders_nothing_in_the_strip() {
    let lay = group_layout(GroupCollapse::Overflow, true);
    assert!(!lay.rendered);
    assert!(!lay.show_label);
}

#[test]
fn estimated_metrics_scale_with_button_count_and_condense_smaller() {
    let two = estimate_group_metrics(1, 2, true);
    let three = estimate_group_metrics(1, 3, true);
    assert_eq!(two.priority, 1);
    // Two buttons + one gap + both side paddings.
    assert_eq!(two.full_px, 2.0 * TOUCH_MIN + 2.0 + 2.0 * SPACE_2);
    // Condensed drops the gap and tightens the padding.
    assert_eq!(two.condensed_px, 2.0 * TOUCH_MIN + 2.0 * SPACE_1);
    assert!(two.condensed_px < two.full_px);
    assert!(three.full_px > two.full_px, "more buttons ⇒ wider");
}

#[test]
fn estimated_metrics_never_underflow_for_an_empty_group() {
    // A zero-button group is treated as one button (never negative gap width).
    let m = estimate_group_metrics(0, 0, false);
    assert_eq!(m.full_px, TOUCH_MIN + 2.0 * SPACE_2);
    assert!(m.condensed_px > 0.0);
}

// ── §11 Partial phase ─────────────────────────────────────────────────────────

/// The `groups()` fixture with the *middle* group (priority 1) opted into
/// Partial at 30 px. Widths per level (collapse order: g0, g1, g2):
/// full 100 each, condensed 50 each, partial(g1) 30.
fn groups_with_partial() -> Vec<GroupMetrics> {
    let mut gs = groups();
    gs[1].partial_px = Some(30.0);
    gs
}

#[test]
fn opted_in_group_passes_through_partial_before_overflowing() {
    let gs = groups_with_partial();
    // All condensed = 150. The partial phase can shave exactly 20 more
    // (g1: 50 → 30); ask for 130 and that is where the cascade must land —
    // levels 4 (g0 no-op) and 5 (g1 partial) applied, nothing overflowed.
    let c = resolve_cascade(&gs, 130.0, 0);
    assert_eq!(
        c.states,
        vec![
            GroupCollapse::Condensed,
            GroupCollapse::Partial,
            GroupCollapse::Condensed,
        ]
    );
    assert!(!c.overflow, "partial is not overflow");
    assert!(!c.scroll);

    // One pixel less than 130 cannot be served by the partial phase; the
    // overflow phase must begin (g0 out; its 50 px leaves g1 partial + g2
    // condensed + the More button).
    let c = resolve_cascade(&gs, 129.0, 0);
    assert_eq!(c.states[0], GroupCollapse::Overflow);
    assert!(c.overflow);
}

#[test]
fn partial_width_is_counted_and_is_never_wider_than_condensed() {
    let m = estimate_partial_metrics(1, 6, 3, true);
    let partial = m.partial_px.expect("opted in");
    // Retained 3 + the submenu chip at TOUCH_MIN, tight padding.
    assert_eq!(partial, 4.0 * TOUCH_MIN + 2.0 * SPACE_1);
    assert!(partial <= m.condensed_px, "monotonicity invariant");
    // A retained count near the button count clamps to condensed instead of
    // growing the strip (retained 2 of 2 + chip would exceed condensed).
    let clamped = estimate_partial_metrics(1, 2, 2, false);
    assert_eq!(
        clamped.partial_px.expect("opted"),
        clamped.condensed_px,
        "clamped to condensed"
    );
}

#[test]
fn non_opting_groups_never_show_partial_and_ladder_ends_unchanged() {
    // No opt-in anywhere: sweep the whole ladder by walking prev_level over
    // every level at an unfittable width — Partial must never appear, and the
    // floor is still all-Overflow.
    let gs = groups();
    for prev in 0..=(3 * gs.len()) {
        let c = resolve_cascade(&gs, 1.0, prev);
        assert!(
            !c.states.contains(&GroupCollapse::Partial),
            "prev {prev} produced Partial without opt-in"
        );
    }
    let floor = resolve_cascade(&gs, 1.0, 0);
    assert_eq!(floor.states, vec![GroupCollapse::Overflow; 3]);
    assert!(floor.scroll);
}
