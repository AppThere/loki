// SPDX-License-Identifier: Apache-2.0

//! Tests for the Layout tab's pure margin-preset matching.

use super::{MARGIN_PRESETS, margin_matches};

#[test]
fn no_margins_matches_no_preset() {
    assert!(!margin_matches(None, (72.0, 72.0, 72.0, 72.0)));
}

#[test]
fn exact_margins_match_their_preset() {
    assert!(margin_matches(
        Some((72.0, 72.0, 72.0, 72.0)),
        (72.0, 72.0, 72.0, 72.0)
    ));
    assert!(margin_matches(
        Some((72.0, 72.0, 144.0, 144.0)),
        (72.0, 72.0, 144.0, 144.0)
    ));
}

#[test]
fn near_equal_margins_match_within_half_a_point() {
    // Import rounding can leave sub-point drift; a preset still reads as active.
    assert!(margin_matches(
        Some((72.2, 71.8, 72.0, 72.0)),
        (72.0, 72.0, 72.0, 72.0)
    ));
    // A full point off is a different (custom) margin.
    assert!(!margin_matches(
        Some((73.0, 72.0, 72.0, 72.0)),
        (72.0, 72.0, 72.0, 72.0)
    ));
}

#[test]
fn different_presets_do_not_cross_match() {
    // Narrow margins must not read as Normal, and vice versa.
    let normal = (72.0, 72.0, 72.0, 72.0);
    let narrow = (36.0, 36.0, 36.0, 36.0);
    assert!(!margin_matches(Some(narrow), normal));
    assert!(!margin_matches(Some(normal), narrow));
}

#[test]
fn the_presets_are_distinct() {
    // Every preset's (t,b,l,r) is unique, so at most one button is ever active.
    for (i, a) in MARGIN_PRESETS.iter().enumerate() {
        for b in &MARGIN_PRESETS[i + 1..] {
            assert_ne!(
                (a.1, a.2, a.3, a.4),
                (b.1, b.2, b.3, b.4),
                "presets {} and {} collide",
                a.0,
                b.0,
            );
        }
    }
}
