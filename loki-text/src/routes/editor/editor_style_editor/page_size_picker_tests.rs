// SPDX-License-Identifier: Apache-2.0

//! Tests for the custom page-size entry's acceptance rule (Spec 08 T6.2).

use super::{MAX_EDGE_PT, MIN_EDGE_PT, parse_custom_size};
use loki_doc_model::layout::paper_catalog::paper_for;

#[test]
fn a_plain_pair_of_numbers_parses() {
    let s = parse_custom_size("612", "792").expect("parses");
    assert_eq!(s.width.value(), 612.0);
    assert_eq!(s.height.value(), 792.0);
    // And a catalogued size entered by hand is still recognised by name — the
    // custom field is an entry route, not a separate kind of size.
    assert_eq!(paper_for(&s).map(|p| p.id), Some("us-letter"));
}

#[test]
fn decimals_and_surrounding_space_are_accepted() {
    let s = parse_custom_size(" 595.28 ", "841.89").expect("parses");
    assert_eq!(paper_for(&s).map(|p| p.id), Some("a4"));
}

/// Every rejection polarity, each asserted to be `None` — a parser tested only
/// on inputs it accepts reports nothing about what it lets through.
#[test]
fn unusable_entries_are_rejected_rather_than_clamped() {
    for (w, h, why) in [
        ("", "792", "empty width"),
        ("612", "", "empty height"),
        ("wide", "792", "non-numeric"),
        ("-612", "792", "negative"),
        ("0", "792", "zero"),
        ("35", "792", "below the minimum edge"),
        ("612", "35", "below the minimum edge, height"),
        ("14401", "792", "above the maximum edge"),
        ("612", "14401", "above the maximum edge, height"),
        ("inf", "792", "infinite"),
        ("NaN", "792", "not a number"),
    ] {
        assert!(
            parse_custom_size(w, h).is_none(),
            "accepted {why}: {w:?} × {h:?}"
        );
    }
}

/// The bounds are inclusive, so the documented limits are themselves usable —
/// an off-by-one here would make the stated range a lie.
#[test]
fn the_documented_bounds_are_inclusive() {
    let lo = format!("{MIN_EDGE_PT}");
    let hi = format!("{MAX_EDGE_PT}");
    assert!(
        parse_custom_size(&lo, &lo).is_some(),
        "minimum edge rejected"
    );
    assert!(
        parse_custom_size(&hi, &hi).is_some(),
        "maximum edge rejected"
    );
}

/// A custom size the catalogue cannot name stays nameless — this is the case
/// the whole "user-defined" half of T6.2 exists for.
#[test]
fn a_genuinely_custom_size_has_no_catalogue_name() {
    let s = parse_custom_size("500", "700").expect("parses");
    assert!(paper_for(&s).is_none());
}
