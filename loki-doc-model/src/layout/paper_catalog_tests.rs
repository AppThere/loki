// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the paper-size catalogue (Spec 08 T6.2).

use super::{MATCH_TOLERANCE_PT, PAPERS, Paper, paper_by_id, paper_for};
use crate::layout::page::PageSize;
use crate::loki_primitives::units::Points;

fn size(w: f64, h: f64) -> PageSize {
    PageSize {
        width: Points::new(w),
        height: Points::new(h),
    }
}

/// **The assertion the tolerance rests on.** `paper_for` returns the *first*
/// match, so if any two entries could both match one page the answer would
/// depend on catalogue order — a name that changes when a row is moved.
///
/// This inverts the naming predicate: rather than checking that each paper
/// matches itself (which passes for any tolerance, however wide), it checks
/// that no paper matches *another*.
#[test]
fn paper_entries_are_mutually_distinguishable() {
    // `matches` requires *both* axes within tolerance, so a pair is
    // distinguishable exactly when its **better**-separated axis exceeds the
    // tolerance. That max-of-the-two-axes gap, minimised over all pairs, is the
    // only number the tolerance has to stay under — a pair sharing one edge
    // exactly (Folio and Legal both being 612 pt wide) is not a near-miss.
    let axis_gap = |a: &Paper, b: &Paper| {
        let short = |p: &Paper| p.width_pt.min(p.height_pt);
        let long = |p: &Paper| p.width_pt.max(p.height_pt);
        (short(a) - short(b)).abs().max((long(a) - long(b)).abs())
    };
    let mut tightest = f64::MAX;
    let mut tightest_pair = ("", "");
    for (i, a) in PAPERS.iter().enumerate() {
        for b in PAPERS.iter().skip(i + 1) {
            assert!(
                !a.matches(&b.portrait()),
                "{} and {} are within the {MATCH_TOLERANCE_PT} pt match tolerance — \
                 `paper_for` would name a page by whichever comes first",
                a.id,
                b.id
            );
            let gap = axis_gap(a, b);
            if gap < tightest {
                tightest = gap;
                tightest_pair = (a.id, b.id);
            }
        }
    }
    assert!(
        tightest > MATCH_TOLERANCE_PT,
        "closest pair {tightest_pair:?} is separated by only {tightest:.2} pt on its \
         better axis, which is inside the {MATCH_TOLERANCE_PT} pt tolerance"
    );
}

/// Every paper is found by its own dimensions, in **both** orientations.
#[test]
fn every_paper_is_named_from_its_own_dimensions() {
    for p in PAPERS {
        let portrait = p.portrait();
        assert_eq!(
            paper_for(&portrait).map(|f| f.id),
            Some(p.id),
            "{} was not found from its own portrait dimensions",
            p.id
        );
        let landscape = size(portrait.height.value(), portrait.width.value());
        assert_eq!(
            paper_for(&landscape).map(|f| f.id),
            Some(p.id),
            "{} was not found when rotated to landscape",
            p.id
        );
    }
}

/// A size in no catalogue entry has no name — the user-defined case, and the
/// polarity that a lookup returning "nearest paper" would fail.
#[test]
fn an_uncatalogued_size_has_no_name() {
    assert!(paper_for(&size(500.0, 700.0)).is_none());
    // Just outside A4 on one axis is *not* A4.
    let a4 = super::A4.portrait();
    let off = size(a4.width.value() + 2.0, a4.height.value());
    assert!(
        paper_for(&off).is_none(),
        "a page 2 pt wider than A4 was still named A4"
    );
    // ...but within tolerance it is.
    let near = size(a4.width.value() + 0.5, a4.height.value() - 0.5);
    assert_eq!(paper_for(&near).map(|p| p.id), Some("a4"));
}

/// The catalogue is the definition `PageSize`'s two convenience constructors
/// use, so they cannot drift from the row that names them.
#[test]
fn the_convenience_constructors_agree_with_the_catalogue() {
    assert_eq!(paper_for(&PageSize::a4()).map(|p| p.id), Some("a4"));
    assert_eq!(
        paper_for(&PageSize::letter()).map(|p| p.id),
        Some("us-letter")
    );
    // And the historical literals still hold, to the tenth of a point.
    assert!((PageSize::a4().width.value() - 595.28).abs() < 0.1);
    assert!((PageSize::a4().height.value() - 841.89).abs() < 0.1);
    assert_eq!(PageSize::letter().width.value(), 612.0);
    assert_eq!(PageSize::letter().height.value(), 792.0);
}

/// ISO B5 and JIS B5 share a colloquial name and must stay separate entries.
#[test]
fn iso_and_jis_b_series_do_not_collide() {
    let iso = paper_by_id("iso-b5").expect("iso-b5");
    let jis = paper_by_id("jis-b5").expect("jis-b5");
    assert!(!iso.matches(&jis.portrait()));
    assert_eq!(paper_for(&iso.portrait()).map(|p| p.id), Some("iso-b5"));
    assert_eq!(paper_for(&jis.portrait()).map(|p| p.id), Some("jis-b5"));
}

/// Choosing a paper for a landscape page keeps it landscape.
#[test]
fn oriented_like_preserves_the_page_orientation() {
    let landscape_letter = size(792.0, 612.0);
    let a4 = super::A4.oriented_like(&landscape_letter);
    assert!(
        a4.width.value() > a4.height.value(),
        "picking A4 for a landscape page rotated it to portrait"
    );
    assert_eq!(paper_for(&a4).map(|p| p.id), Some("a4"));

    let portrait_letter = size(612.0, 792.0);
    let a4p = super::A4.oriented_like(&portrait_letter);
    assert!(a4p.width.value() < a4p.height.value());
}

/// Ids are the addressing key for settings and the UI; a duplicate would make
/// `paper_by_id` order-dependent in the same way a dimension clash would.
#[test]
fn ids_are_unique_and_resolvable() {
    for (i, a) in PAPERS.iter().enumerate() {
        for b in PAPERS.iter().skip(i + 1) {
            assert_ne!(a.id, b.id, "duplicate paper id {}", a.id);
        }
        assert_eq!(paper_by_id(a.id).map(|p| p.id), Some(a.id));
    }
    assert!(paper_by_id("not-a-paper").is_none());
}

/// The catalogue covers what T6.2 asked for; a missing row is a silent gap the
/// UI cannot report.
#[test]
fn the_catalogue_covers_the_specified_sizes() {
    for id in [
        "a0",
        "a1",
        "a2",
        "a3",
        "a4",
        "a5",
        "a6",
        "iso-b4",
        "iso-b5",
        "iso-b6",
        "jis-b4",
        "jis-b5",
        "jis-b6",
        "c5",
        "c6",
        "dl",
        "us-letter",
        "us-legal",
        "tabloid",
        "executive",
        "statement",
        "folio",
        "quarto",
        "env-10",
        "env-monarch",
        "index-3x5",
        "index-4x6",
        "index-5x8",
    ] {
        assert!(paper_by_id(id).is_some(), "catalogue is missing {id}");
    }
}
