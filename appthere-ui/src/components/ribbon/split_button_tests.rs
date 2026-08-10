// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Pure-logic tests for the split-button menu: row navigation over a caller
//! -supplied count, and the placement request.

use super::{next_row, prev_row, split_menu_placement};
use crate::components::popover::{Align, Rect, Side};

#[test]
fn next_wraps_and_enters_at_the_top() {
    assert_eq!(next_row(None, 3), 0, "first Down lands on row 0");
    assert_eq!(next_row(Some(0), 3), 1);
    assert_eq!(next_row(Some(2), 3), 0, "wraps");
}

#[test]
fn prev_wraps_and_enters_at_the_bottom() {
    assert_eq!(prev_row(None, 3), 2, "first Up reaches the last row");
    assert_eq!(prev_row(Some(2), 3), 1);
    assert_eq!(prev_row(Some(0), 3), 2, "wraps");
}

#[test]
fn navigation_survives_degenerate_counts() {
    // An empty menu cannot be navigated into anything but row 0 / nothing —
    // the guards must not divide by zero or underflow.
    assert_eq!(next_row(None, 0), 0);
    assert_eq!(prev_row(None, 0), 0);
    assert_eq!(next_row(Some(0), 1), 0);
    assert_eq!(prev_row(Some(0), 1), 0);
}

#[test]
fn menu_hangs_below_and_end_aligned() {
    let anchor = Rect {
        x: 100.0,
        y: 10.0,
        width: 44.0,
        height: 44.0,
    };
    let req = split_menu_placement(anchor);
    assert_eq!(req.preferred, Side::Below);
    assert_eq!(req.align, Align::End);
    assert_eq!(req.anchor, anchor);
    assert!(req.width > 0.0 && req.height > 0.0);
}
