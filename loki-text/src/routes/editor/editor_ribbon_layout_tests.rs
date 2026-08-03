// SPDX-License-Identifier: Apache-2.0

//! Tests for the Layout tab's pure margin- and page-size-preset matching.

use super::{MARGIN_PRESETS, PAGE_SIZE_PRESETS, margin_matches, page_size_matches};
use loki_doc_model::layout::paper_catalog::{self, Paper};

/// A page's `(width, height)` in points, as the ribbon receives it.
fn dims(p: &Paper) -> (f64, f64) {
    (p.width_pt, p.height_pt)
}

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

const A4: &Paper = &paper_catalog::A4;
const LETTER: &Paper = &paper_catalog::US_LETTER;

#[test]
fn no_page_size_matches_no_preset() {
    assert!(!page_size_matches(None, A4));
}

#[test]
fn page_size_matches_regardless_of_orientation() {
    // Portrait A4 and landscape A4 (swapped) both read as A4.
    let (w, h) = dims(A4);
    assert!(page_size_matches(Some((w, h)), A4));
    assert!(page_size_matches(Some((h, w)), A4), "landscape A4 still A4");
}

#[test]
fn a4_and_letter_do_not_cross_match() {
    assert!(!page_size_matches(Some(dims(LETTER)), A4));
    assert!(!page_size_matches(Some(dims(A4)), LETTER));
    assert!(page_size_matches(Some(dims(LETTER)), LETTER));
}

#[test]
fn sub_point_drift_still_matches() {
    // Import rounding leaves sub-point drift; the page still reads as A4.
    assert!(page_size_matches(Some((595.0, 842.0)), A4));
    // A whole 2 pt off is a different (user-defined) size.
    assert!(!page_size_matches(Some((597.28, 841.89)), A4));
}

/// The ribbon's quick presets are catalogue rows, not literals — so a preset
/// button can never point at a size the catalogue would decline to name, which
/// is what a private copy of the dimensions allowed.
#[test]
fn every_ribbon_preset_is_the_paper_it_claims() {
    for (aria, paper, _icon) in PAGE_SIZE_PRESETS.iter().copied() {
        assert!(
            page_size_matches(Some(dims(paper)), paper),
            "{aria} does not match its own paper"
        );
        assert_eq!(
            paper_catalog::paper_for(&paper.portrait()).map(|p| p.id),
            Some(paper.id),
            "{aria} points at a paper the catalogue does not name"
        );
    }
}
