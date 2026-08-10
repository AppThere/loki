// SPDX-License-Identifier: Apache-2.0

//! Tests for the publish dialog's tab set and options.

use super::*;
use appthere_ui::DialogTabLayout;
use appthere_ui::responsive::Breakpoint;

#[test]
fn index_and_from_index_round_trip_for_every_tab() {
    for (i, tab) in PublishTab::ALL.iter().enumerate() {
        assert_eq!(tab.index(), i);
        assert_eq!(PublishTab::from_index(i), *tab);
    }
    assert_eq!(PublishTab::from_index(99), PublishTab::Output, "saturates");
}

#[test]
fn labels_match_the_strip_order_and_length() {
    let labels = PublishTab::labels();
    assert_eq!(labels.len(), PublishTab::ALL.len());
    for (i, tab) in PublishTab::ALL.iter().enumerate() {
        assert_eq!(labels[i], tab.label());
    }
}

#[test]
fn the_medium_inline_count_collapses_the_strip() {
    assert!(PublishTab::INLINE_AT_MEDIUM < PublishTab::ALL.len());
    assert_eq!(
        DialogTabLayout::for_breakpoint(
            Breakpoint::Medium,
            PublishTab::ALL.len(),
            PublishTab::INLINE_AT_MEDIUM
        ),
        DialogTabLayout::Overflow {
            visible: 3,
            overflow: 2
        }
    );
}

/// Three levels is the default a reader expects; the design shows it selected.
#[test]
fn the_default_toc_depth_is_three_levels() {
    assert_eq!(TocDepth::default(), TocDepth(3));
    assert_eq!(TocDepth::default().index(), 2);
}

#[test]
fn toc_depths_round_trip_through_their_index() {
    for (i, depth) in TocDepth::CHOICES.iter().enumerate() {
        assert_eq!(TocDepth(*depth).index(), i);
        assert_eq!(TocDepth::from_index(i), TocDepth(*depth));
    }
    assert_eq!(TocDepth::from_index(99), TocDepth(6), "saturates");
}

/// An out-of-range depth from a restored setting must land somewhere sane
/// rather than selecting nothing.
#[test]
fn an_out_of_range_depth_falls_back_to_the_default_slot() {
    assert_eq!(TocDepth(9).index(), 2);
    assert_eq!(TocDepth(0).index(), 2);
}

/// Every depth has its own label, or two choices would be indistinguishable.
#[test]
fn every_depth_has_a_distinct_label() {
    let labels: Vec<String> = TocDepth::CHOICES
        .iter()
        .map(|d| TocDepth::label(*d))
        .collect();
    for (i, a) in labels.iter().enumerate() {
        for b in &labels[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
