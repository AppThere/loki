// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::DialogTabLayout`] — the tab-strip collapse rules.

use super::*;

/// The paragraph style editor: seven tabs, three inline at Medium.
const TOTAL: usize = 7;
const INLINE: usize = 3;

#[test]
fn each_size_class_picks_a_different_presentation() {
    assert_eq!(
        DialogTabLayout::for_breakpoint(Breakpoint::Expanded, TOTAL, INLINE),
        DialogTabLayout::Inline
    );
    assert_eq!(
        DialogTabLayout::for_breakpoint(Breakpoint::Medium, TOTAL, INLINE),
        DialogTabLayout::Overflow {
            visible: 3,
            overflow: 4
        },
        "design note 03: keep 3, collapse 4"
    );
    assert_eq!(
        DialogTabLayout::for_breakpoint(Breakpoint::Compact, TOTAL, INLINE),
        DialogTabLayout::Picker
    );
}

/// Collapsing a single tab into a menu costs a click and a menu to save one
/// label. The guard must be *false* at the first total that genuinely overflows,
/// or it is a description rather than a precondition.
#[test]
fn a_single_overflowing_tab_stays_inline() {
    for total in 0..=INLINE + 1 {
        assert_eq!(
            DialogTabLayout::for_breakpoint(Breakpoint::Medium, total, INLINE),
            DialogTabLayout::Inline,
            "{total} tabs do not earn a menu"
        );
    }
    assert!(
        matches!(
            DialogTabLayout::for_breakpoint(Breakpoint::Medium, INLINE + 2, INLINE),
            DialogTabLayout::Overflow { .. }
        ),
        "two overflowing tabs do"
    );
}

/// A strip showing no tabs beside a `More ▾ 7` menu is worse than an
/// overflowing row of labels.
#[test]
fn zero_inline_slots_degrade_to_inline_not_an_empty_strip() {
    assert_eq!(
        DialogTabLayout::for_breakpoint(Breakpoint::Medium, TOTAL, 0),
        DialogTabLayout::Inline
    );
}

/// Expanded and Compact ignore the inline count entirely — it is a Medium-only
/// parameter.
#[test]
fn inline_count_only_affects_medium() {
    for inline in [0, 1, 3, 99] {
        assert_eq!(
            DialogTabLayout::for_breakpoint(Breakpoint::Expanded, TOTAL, inline),
            DialogTabLayout::Inline
        );
        assert_eq!(
            DialogTabLayout::for_breakpoint(Breakpoint::Compact, TOTAL, inline),
            DialogTabLayout::Picker
        );
    }
}

#[test]
fn inline_layout_shows_every_tab_and_overflows_none() {
    let layout = DialogTabLayout::Inline;
    assert_eq!(layout.inline_indices(TOTAL, 0), vec![0, 1, 2, 3, 4, 5, 6]);
    assert!(layout.overflow_indices(TOTAL, 0).is_empty());
}

#[test]
fn picker_layout_shows_no_inline_tabs_and_holds_them_all() {
    let layout = DialogTabLayout::Picker;
    assert!(layout.inline_indices(TOTAL, 2).is_empty());
    assert_eq!(
        layout.overflow_indices(TOTAL, 2),
        vec![0, 1, 2, 3, 4, 5, 6],
        "the picker lists the whole set — nothing is dropped at Compact"
    );
}

/// An already-inline active tab must not perturb the strip.
#[test]
fn overflow_keeps_the_leading_slots_when_the_active_tab_is_inline() {
    let layout = DialogTabLayout::for_breakpoint(Breakpoint::Medium, TOTAL, INLINE);
    for active in 0..INLINE {
        assert_eq!(layout.inline_indices(TOTAL, active), vec![0, 1, 2]);
        assert_eq!(layout.overflow_indices(TOTAL, active), vec![3, 4, 5, 6]);
    }
}

/// The rule that keeps the strip honest: an overflowing active tab is pulled
/// into the last slot, so the body's owning tab is never hidden behind a menu.
#[test]
fn an_overflowing_active_tab_displaces_the_last_inline_slot() {
    let layout = DialogTabLayout::for_breakpoint(Breakpoint::Medium, TOTAL, INLINE);
    assert_eq!(layout.inline_indices(TOTAL, 6), vec![0, 1, 6]);
    assert_eq!(layout.overflow_indices(TOTAL, 6), vec![2, 3, 4, 5]);
}

/// The active tab is reachable without opening a menu at every size class and
/// for every tab — the property the displacement rule exists to guarantee.
#[test]
fn the_active_tab_is_always_inline_or_in_the_picker() {
    for bp in [Breakpoint::Expanded, Breakpoint::Medium] {
        let layout = DialogTabLayout::for_breakpoint(bp, TOTAL, INLINE);
        for active in 0..TOTAL {
            assert!(
                layout.inline_indices(TOTAL, active).contains(&active),
                "{bp:?} hides active tab {active} behind the menu"
            );
        }
    }
}

/// The strip must not change width as the user moves between tabs, so a
/// displaced active tab replaces a slot rather than adding one.
#[test]
fn inline_slot_count_is_constant_across_every_active_tab() {
    let layout = DialogTabLayout::for_breakpoint(Breakpoint::Medium, TOTAL, INLINE);
    for active in 0..TOTAL {
        assert_eq!(layout.inline_indices(TOTAL, active).len(), INLINE);
    }
}

/// Inline and overflow partition the tab set: every tab appears exactly once,
/// so no tab is lost and none is offered twice.
#[test]
fn inline_and_overflow_partition_the_tab_set() {
    for bp in [
        Breakpoint::Expanded,
        Breakpoint::Medium,
        Breakpoint::Compact,
    ] {
        let layout = DialogTabLayout::for_breakpoint(bp, TOTAL, INLINE);
        for active in 0..TOTAL {
            let mut all = layout.inline_indices(TOTAL, active);
            all.extend(layout.overflow_indices(TOTAL, active));
            all.sort_unstable();
            assert_eq!(
                all,
                (0..TOTAL).collect::<Vec<_>>(),
                "{bp:?} active={active} does not partition the tabs"
            );
        }
    }
}

/// An out-of-range active index must not panic or corrupt the partition — the
/// caller's tab enum and the strip's length can disagree during a redraw.
#[test]
fn an_out_of_range_active_index_is_ignored() {
    let layout = DialogTabLayout::for_breakpoint(Breakpoint::Medium, TOTAL, INLINE);
    assert_eq!(layout.inline_indices(TOTAL, 99), vec![0, 1, 2]);
    assert_eq!(layout.overflow_indices(TOTAL, 99), vec![3, 4, 5, 6]);
}

/// Fewer tabs than inline slots must not index past the end.
#[test]
fn fewer_tabs_than_slots_does_not_overrun() {
    let layout = DialogTabLayout::Overflow {
        visible: 3,
        overflow: 0,
    };
    assert_eq!(layout.inline_indices(2, 0), vec![0, 1]);
    assert!(layout.overflow_indices(2, 0).is_empty());
}
