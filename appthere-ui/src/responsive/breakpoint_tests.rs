// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::Breakpoint;
use crate::tokens::layout::{BREAKPOINT_COMPACT_MAX_PX, BREAKPOINT_EXPANDED_MIN_PX};

// Spec 03 M1 acceptance: a width change produces the correct `Breakpoint` with
// no real window — pure classification, mirroring the Spec 02 harness style.

#[test]
fn well_inside_each_band_classifies_correctly() {
    assert_eq!(Breakpoint::from_width(0.0), Breakpoint::Compact);
    assert_eq!(Breakpoint::from_width(375.0), Breakpoint::Compact);
    assert_eq!(Breakpoint::from_width(800.0), Breakpoint::Medium);
    assert_eq!(Breakpoint::from_width(1440.0), Breakpoint::Expanded);
}

#[test]
fn boundaries_are_compact_lower_inclusive_expanded_lower_inclusive() {
    // Compact is `< 600`; exactly 600 is the first Medium width.
    assert_eq!(
        Breakpoint::from_width(BREAKPOINT_COMPACT_MAX_PX - 0.01),
        Breakpoint::Compact
    );
    assert_eq!(
        Breakpoint::from_width(BREAKPOINT_COMPACT_MAX_PX),
        Breakpoint::Medium
    );
    // Medium is `< 1024`; exactly 1024 is the first Expanded width.
    assert_eq!(
        Breakpoint::from_width(BREAKPOINT_EXPANDED_MIN_PX - 0.01),
        Breakpoint::Medium
    );
    assert_eq!(
        Breakpoint::from_width(BREAKPOINT_EXPANDED_MIN_PX),
        Breakpoint::Expanded
    );
}

#[test]
fn variants_order_compact_lt_medium_lt_expanded() {
    // Classify ascending widths and assert the derived classes are monotonic —
    // exercises the `Ord` derivation through runtime values.
    let classes: Vec<Breakpoint> = [100.0, 400.0, 700.0, 1100.0, 2000.0]
        .into_iter()
        .map(Breakpoint::from_width)
        .collect();
    assert!(classes.windows(2).all(|w| w[0] <= w[1]), "{classes:?}");
    // "at least Medium" is a plain comparison.
    assert!(Breakpoint::from_width(800.0) >= Breakpoint::Medium);
    assert!(Breakpoint::from_width(400.0) < Breakpoint::Medium);
}

#[test]
fn predicates_match_the_class() {
    let c = Breakpoint::Compact;
    assert!(c.is_compact() && !c.is_medium() && !c.is_expanded());
    assert!(c.is_touch_first());

    let m = Breakpoint::Medium;
    assert!(m.is_medium() && !m.is_compact() && !m.is_expanded());
    assert!(!m.is_touch_first());

    let e = Breakpoint::Expanded;
    assert!(e.is_expanded() && !e.is_compact() && !e.is_medium());
    assert!(!e.is_touch_first());
}
