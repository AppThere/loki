// SPDX-License-Identifier: Apache-2.0

//! Tests for the custom page-size entry's acceptance rule (Spec 08 T6.2) and
//! its unit handling (T6.4).

use super::{MAX_EDGE_PT, MIN_EDGE_PT, parse_custom_size};
use loki_doc_model::layout::paper_catalog::{PAPERS, paper_for};
use loki_doc_model::loki_primitives::units::MeasurementUnit;

const PT: MeasurementUnit = MeasurementUnit::Point;
const MM: MeasurementUnit = MeasurementUnit::Millimeter;
const IN: MeasurementUnit = MeasurementUnit::Inch;

#[test]
fn a_plain_pair_of_numbers_parses_in_the_active_unit() {
    let s = parse_custom_size("612", "792", PT).expect("parses");
    assert_eq!(s.width.value(), 612.0);
    assert_eq!(s.height.value(), 792.0);
    // A catalogued size entered by hand is still recognised by name — the custom
    // field is an entry route, not a separate kind of size.
    assert_eq!(paper_for(&s).map(|p| p.id), Some("us-letter"));
}

/// **The unit is load-bearing, not decoration.** The same digits typed under two
/// settings must give two different pages; a parser that ignored the unit would
/// pass a test that only ever used one.
#[test]
fn the_same_digits_mean_different_pages_in_different_units() {
    let as_pt = parse_custom_size("210", "297", PT).expect("parses");
    let as_mm = parse_custom_size("210", "297", MM).expect("parses");
    assert!(
        (as_pt.width.value() - as_mm.width.value()).abs() > 100.0,
        "the active unit was ignored"
    );
    // 210 × 297 mm *is* A4 — the metric user types the number they know.
    assert_eq!(paper_for(&as_mm).map(|p| p.id), Some("a4"));
    // 8.5 × 11 in is US Letter, likewise.
    let as_in = parse_custom_size("8.5", "11", IN).expect("parses");
    assert_eq!(paper_for(&as_in).map(|p| p.id), Some("us-letter"));
}

/// An explicit suffix beats the active unit, so a user who knows the number in
/// another unit is not made to convert it by hand.
#[test]
fn an_explicit_suffix_overrides_the_active_unit() {
    let s = parse_custom_size("8.5in", "11in", MM).expect("parses");
    assert_eq!(paper_for(&s).map(|p| p.id), Some("us-letter"));
}

#[test]
fn decimals_and_surrounding_space_are_accepted() {
    let s = parse_custom_size(" 595.28 ", "841.89", PT).expect("parses");
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
        ("1 furlong", "792", "unrecognised suffix"),
    ] {
        assert!(
            parse_custom_size(w, h, PT).is_none(),
            "accepted {why}: {w:?} × {h:?}"
        );
    }
}

/// **The range is a property of the page, not of the number typed.** The bounds
/// are checked after conversion, so the same physical page is accepted or
/// rejected identically however it was entered. Checking the raw number instead
/// would reject every millimetre entry — A4 is 210 mm, well under a floor of 36
/// — and wave through absurd inch ones.
#[test]
fn the_range_is_applied_to_the_page_not_to_the_typed_number() {
    assert!(
        parse_custom_size("210", "297", MM).is_some(),
        "a millimetre entry was measured against a point range"
    );
    // Half an inch is exactly the floor and is accepted; a quarter inch is not.
    assert!(parse_custom_size("0.5", "0.5", IN).is_some());
    assert!(parse_custom_size("0.25", "11", IN).is_none());
    // 200 in is exactly the ceiling; 201 in is over it.
    assert!(parse_custom_size("200", "200", IN).is_some());
    assert!(parse_custom_size("201", "11", IN).is_none());
}

/// The bounds are inclusive, so the documented limits are themselves usable —
/// an off-by-one here would make the stated range a lie.
#[test]
fn the_documented_bounds_are_inclusive() {
    let lo = format!("{MIN_EDGE_PT}");
    let hi = format!("{MAX_EDGE_PT}");
    assert!(
        parse_custom_size(&lo, &lo, PT).is_some(),
        "minimum edge rejected"
    );
    assert!(
        parse_custom_size(&hi, &hi, PT).is_some(),
        "maximum edge rejected"
    );
}

/// A custom size the catalogue cannot name stays nameless — the case the
/// "user-defined" half of T6.2 exists for.
#[test]
fn a_genuinely_custom_size_has_no_catalogue_name() {
    let s = parse_custom_size("500", "700", PT).expect("parses");
    assert!(paper_for(&s).is_none());
}

/// The field seeds itself with `format_bare` and reads back through
/// `parse_custom_size`. Those two must agree **in every unit**, or opening the
/// panel and pressing Set without typing anything would resize the page — a
/// defect that only appears in units the developer does not run in.
#[test]
fn seeding_then_reading_back_returns_the_same_page() {
    for unit in MeasurementUnit::ALL.iter().copied() {
        for paper in PAPERS {
            let size = paper.portrait();
            let (w, h) = (unit.format_bare(size.width), unit.format_bare(size.height));
            let Some(back) = parse_custom_size(&w, &h, unit) else {
                // Papers under the entry floor are legitimately unreachable
                // through this field; the catalogue buttons still set them.
                assert!(
                    size.width.value() < MIN_EDGE_PT || size.height.value() < MIN_EDGE_PT,
                    "{} ({unit:?}) seeded as {w:?} × {h:?} and would not parse back",
                    paper.id
                );
                continue;
            };
            assert_eq!(
                paper_for(&back).map(|p| p.id),
                Some(paper.id),
                "{} ({unit:?}) seeded as {w:?} × {h:?} and came back as something else",
                paper.id
            );
        }
    }
}
