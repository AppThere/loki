// SPDX-License-Identifier: Apache-2.0

//! Tests for [`super::ParaTab`].

use super::*;
use appthere_ui::DialogTabLayout;
use appthere_ui::responsive::Breakpoint;

/// `index` and `from_index` must be inverses, or the strip selects a different
/// tab from the one the user clicked.
#[test]
fn index_and_from_index_round_trip_for_every_tab() {
    for (i, tab) in ParaTab::ALL.iter().enumerate() {
        assert_eq!(tab.index(), i);
        assert_eq!(ParaTab::from_index(i), *tab);
    }
}

/// The index arrives from the strip, whose label vector could disagree with
/// this enum for one frame during a redraw — so it saturates instead of
/// panicking.
#[test]
fn an_out_of_range_index_saturates_rather_than_panicking() {
    assert_eq!(ParaTab::from_index(7), ParaTab::TabStops);
    assert_eq!(ParaTab::from_index(usize::MAX), ParaTab::TabStops);
}

/// The strip renders `labels()`, so its length and order must match the enum or
/// clicking tab *n* opens tab *m*.
#[test]
fn labels_match_the_strip_order_and_length() {
    let labels = ParaTab::labels();
    assert_eq!(labels.len(), ParaTab::ALL.len());
    for (i, tab) in ParaTab::ALL.iter().enumerate() {
        assert_eq!(labels[i], tab.label());
    }
}

/// The design shows seven tabs; a tab added to the enum without a strip slot
/// would silently never render.
#[test]
fn the_dialog_has_seven_tabs() {
    assert_eq!(ParaTab::ALL.len(), 7);
}

/// The preview is suppressed on exactly the two tabs that carry their own
/// visualisation. Asserting only the suppressed pair would pass for a rail that
/// never showed at all.
#[test]
fn preview_shows_everywhere_except_general_and_tab_stops() {
    for tab in [ParaTab::General, ParaTab::TabStops] {
        assert!(!tab.has_preview(), "{tab:?} has its own visualisation");
    }
    for tab in [
        ParaTab::Font,
        ParaTab::Indents,
        ParaTab::Alignment,
        ParaTab::TextFlow,
        ParaTab::Borders,
    ] {
        assert!(tab.has_preview(), "{tab:?} needs the paper preview");
    }
}

/// The inline count must genuinely collapse this tab set at Medium — a value
/// that happened to equal the tab count would silently disable the overflow.
#[test]
fn the_medium_inline_count_actually_collapses_this_tab_set() {
    assert!(ParaTab::INLINE_AT_MEDIUM < ParaTab::ALL.len());
    let layout = DialogTabLayout::for_breakpoint(
        Breakpoint::Medium,
        ParaTab::ALL.len(),
        ParaTab::INLINE_AT_MEDIUM,
    );
    assert_eq!(
        layout,
        DialogTabLayout::Overflow {
            visible: 3,
            overflow: 4
        }
    );
}

/// The tabs the user opens this dialog for keep their inline slots.
#[test]
fn the_inline_slots_hold_the_identity_and_character_tabs() {
    let layout = DialogTabLayout::for_breakpoint(
        Breakpoint::Medium,
        ParaTab::ALL.len(),
        ParaTab::INLINE_AT_MEDIUM,
    );
    let inline = layout.inline_indices(ParaTab::ALL.len(), ParaTab::General.index());
    let tabs: Vec<ParaTab> = inline.into_iter().map(ParaTab::from_index).collect();
    assert_eq!(
        tabs,
        vec![ParaTab::General, ParaTab::Font, ParaTab::Indents]
    );
}

/// Every tab must be reachable at every size class — the feature-parity promise
/// the design makes ("nothing is dropped, only re-laid-out").
#[test]
fn every_tab_is_reachable_at_every_size_class() {
    let total = ParaTab::ALL.len();
    for bp in [
        Breakpoint::Compact,
        Breakpoint::Medium,
        Breakpoint::Expanded,
    ] {
        let layout = DialogTabLayout::for_breakpoint(bp, total, ParaTab::INLINE_AT_MEDIUM);
        for active in 0..total {
            let mut reachable = layout.inline_indices(total, active);
            reachable.extend(layout.overflow_indices(total, active));
            reachable.sort_unstable();
            assert_eq!(reachable, (0..total).collect::<Vec<_>>(), "{bp:?}");
        }
    }
}
