// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the autofit min-content width distribution.

use super::distribute_with_mins;

fn sum(v: &[f32]) -> f32 {
    v.iter().sum()
}

#[test]
fn no_violation_returns_scaled_unchanged() {
    // Every column already meets its minimum → proportional result is kept, so
    // well-proportioned tables are unaffected by the min-content guarantee.
    let scaled = [200.0, 400.0];
    let mins = [50.0, 100.0];
    let out = distribute_with_mins(&scaled, &mins, 600.0);
    assert_eq!(out, vec![200.0, 400.0]);
}

#[test]
fn narrow_column_widens_to_min_and_others_absorb_it() {
    // The classic callout: a tiny preferred label column (20) whose content
    // needs 80. It must widen to 80; the wide body column gives up the 60,
    // and the total table width is preserved.
    let scaled = [20.0, 580.0];
    let mins = [80.0, 100.0];
    let out = distribute_with_mins(&scaled, &mins, 600.0);
    assert!(
        (out[0] - 80.0).abs() < 0.5,
        "label pinned to its min: {out:?}"
    );
    assert!(
        (out[1] - 520.0).abs() < 0.5,
        "body absorbs the deficit: {out:?}"
    );
    assert!((sum(&out) - 600.0).abs() < 0.5, "total preserved: {out:?}");
}

#[test]
fn mins_exceeding_table_width_overflow_at_minimums() {
    // When the minimums alone don't fit, every column keeps its minimum and the
    // table overflows — Word's behaviour, not a rescale-to-fit.
    let scaled = [100.0, 100.0];
    let mins = [250.0, 250.0];
    let out = distribute_with_mins(&scaled, &mins, 300.0);
    assert!((out[0] - 250.0).abs() < 0.5, "{out:?}");
    assert!((out[1] - 250.0).abs() < 0.5, "{out:?}");
}

#[test]
fn multiple_narrow_columns_each_reach_min() {
    // Two under-min columns and one generous column: both narrows reach their
    // min, the generous one absorbs the combined deficit.
    let scaled = [10.0, 10.0, 580.0];
    let mins = [70.0, 90.0, 50.0];
    let out = distribute_with_mins(&scaled, &mins, 600.0);
    assert!(out[0] + 0.5 >= 70.0, "{out:?}");
    assert!(out[1] + 0.5 >= 90.0, "{out:?}");
    assert!((sum(&out) - 600.0).abs() < 1.0, "total preserved: {out:?}");
}
