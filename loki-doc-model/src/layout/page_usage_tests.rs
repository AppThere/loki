// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The ODF codec and the one question the paginator asks of it.

use super::PageUsage;

/// **Every value round-trips**, so the writer and the reader cannot disagree
/// about a spelling. Stated over the whole enum rather than over a sample: a
/// value added later without a codec arm would otherwise pass.
#[test]
fn every_usage_round_trips_through_its_odf_spelling() {
    for usage in [
        PageUsage::All,
        PageUsage::Mirrored,
        PageUsage::Left,
        PageUsage::Right,
    ] {
        assert_eq!(
            PageUsage::from_odf(usage.as_odf()),
            usage,
            "{usage:?} did not survive its own spelling ({})",
            usage.as_odf(),
        );
    }
}

/// **An unreadable value opens the document.** ODF's own default for the
/// attribute is `all`, and a page usage nobody can parse is not a reason to
/// refuse a file.
#[test]
fn an_unknown_usage_is_all_rather_than_an_error() {
    for junk in ["", "sideways", "MIRRORED", "all "] {
        assert_eq!(PageUsage::from_odf(junk), PageUsage::All, "{junk:?}");
    }
}

/// **Only `Mirrored` swaps margins**, and the other three are the polarity that
/// makes that mean something.
///
/// `Left` and `Right` restrict *which* pages a layout is used for; they do not
/// alternate margins within it. Reading either as "mirrored" would swap the
/// margins of a single-sided document on every even page — a defect that looks
/// like a layout engine bug rather than a misread attribute.
#[test]
fn left_and_right_select_pages_rather_than_swapping_margins() {
    assert!(PageUsage::Mirrored.mirrors_margins());
    for usage in [PageUsage::All, PageUsage::Left, PageUsage::Right] {
        assert!(
            !usage.mirrors_margins(),
            "{usage:?} must not alternate margins",
        );
    }
}

/// The default is the non-mirroring one — a document that says nothing about
/// page usage is single-sided.
#[test]
fn the_default_is_all() {
    assert_eq!(PageUsage::default(), PageUsage::All);
    assert!(!PageUsage::default().mirrors_margins());
}
