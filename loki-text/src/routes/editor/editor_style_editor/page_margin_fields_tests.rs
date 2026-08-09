// SPDX-License-Identifier: Apache-2.0

//! Tests for the margin entry's acceptance rule and unit handling (Spec 08
//! T6.7 / T6.4).

use super::{MAX_MARGIN_PT, MIN_MARGIN_PT, margin_field_key, parse_margins};
use loki_doc_model::layout::page::{PageLayout, PageMargins};
use loki_doc_model::loki_primitives::units::{MeasurementUnit, Points};

const PT: MeasurementUnit = MeasurementUnit::Point;
const MM: MeasurementUnit = MeasurementUnit::Millimeter;
const IN: MeasurementUnit = MeasurementUnit::Inch;

fn four(a: &str, b: &str, c: &str, d: &str) -> [String; 4] {
    [a.into(), b.into(), c.into(), d.into()]
}

/// A base whose header/footer/gutter are all distinct and none of them 0 or 72,
/// so anything that leaked an edge into them — or reset them to a default —
/// is visible rather than coincidentally right.
fn base() -> PageMargins {
    PageMargins {
        header: Points::new(11.0),
        footer: Points::new(22.0),
        gutter: Points::new(33.0),
        ..PageMargins::default()
    }
}

/// **Each field reaches its own edge.** Four boxes written four times is an
/// invitation to read one into the next; the four values here are pairwise
/// distinct, so any transposition changes the answer.
#[test]
fn each_entry_lands_on_the_edge_it_is_labelled_with() {
    let m = parse_margins(&four("10", "20", "30", "40"), PT, &base()).expect("parses");
    assert_eq!(m.top.value(), 10.0);
    assert_eq!(m.bottom.value(), 20.0);
    assert_eq!(m.left.value(), 30.0);
    assert_eq!(m.right.value(), 40.0);
}

/// **Header, footer and gutter survive a margin edit.** They are `PageMargins`
/// fields the presets do not touch and this row does not show; building a fresh
/// struct instead of spreading `base` would silently reset all three to the
/// `Default` values — 36 / 36 / 0 — while the user thought they set a margin.
#[test]
fn the_fields_this_row_does_not_show_are_carried_through() {
    let m = parse_margins(&four("10", "20", "30", "40"), PT, &base()).expect("parses");
    assert_eq!(m.header.value(), 11.0, "header was reset by a margin edit");
    assert_eq!(m.footer.value(), 22.0, "footer was reset by a margin edit");
    assert_eq!(m.gutter.value(), 33.0, "gutter was reset by a margin edit");
    // And the guard: the base really did differ from the default, so the
    // assertions above are not three tautologies.
    let d = PageMargins::default();
    assert!(
        d.header.value() != 11.0 && d.footer.value() != 22.0 && d.gutter.value() != 33.0,
        "fixture matches the default, so a reset would be invisible"
    );
}

/// **The unit is load-bearing.** The same digits under two settings must give
/// two different pages; a parser that ignored the unit would pass a test that
/// only ever used one.
#[test]
fn the_same_digits_mean_different_margins_in_different_units() {
    let as_pt = parse_margins(&four("20", "20", "20", "20"), PT, &base()).expect("parses");
    let as_mm = parse_margins(&four("20", "20", "20", "20"), MM, &base()).expect("parses");
    assert!(
        (as_pt.top.value() - as_mm.top.value()).abs() > 30.0,
        "the active unit was ignored"
    );
    // 1 in is 72 pt, whichever way it was typed.
    let as_in = parse_margins(&four("1", "1", "1", "1"), IN, &base()).expect("parses");
    assert!((as_in.top.value() - 72.0).abs() < 0.01);
}

/// An explicit suffix beats the active unit, matching the size field.
#[test]
fn an_explicit_suffix_overrides_the_active_unit() {
    let m = parse_margins(&four("1in", "1in", "1in", "1in"), MM, &base()).expect("parses");
    assert!(
        (m.top.value() - 72.0).abs() < 0.01,
        "an explicit `in` was read as millimetres"
    );
}

/// **The range is checked in points, after conversion** — the limits belong to
/// the page, not to the unit it was typed in. Both ends reject rather than
/// clamp, so a typo cannot become a different page.
#[test]
fn out_of_range_entries_are_refused_in_every_unit() {
    assert!(
        parse_margins(&four("-1", "10", "10", "10"), PT, &base()).is_none(),
        "a negative margin was accepted"
    );
    assert!(
        parse_margins(&four("10", "10", "10", "99999"), PT, &base()).is_none(),
        "an unbounded margin was accepted"
    );
    // 21 in exceeds MAX_MARGIN_PT (20 in) once converted, though "21" is a
    // perfectly ordinary number of points or millimetres.
    assert!(
        parse_margins(&four("21", "1", "1", "1"), IN, &base()).is_none(),
        "the range was checked before the unit conversion"
    );
    assert!(
        parse_margins(&four("21", "1", "1", "1"), PT, &base()).is_some(),
        "21 pt is inside the range and was refused"
    );

    // The inverse — both ends of the range are themselves accepted, so the
    // rejections above are the bound and not a blanket refusal.
    let lo = MIN_MARGIN_PT.to_string();
    let hi = MAX_MARGIN_PT.to_string();
    assert!(parse_margins(&four(&lo, &lo, &lo, &lo), PT, &base()).is_some());
    assert!(parse_margins(&four(&hi, &hi, &hi, &hi), PT, &base()).is_some());
}

/// Anything that is not a number is refused, one bad field is enough, and the
/// refusal is what withholds the Set button.
#[test]
fn an_unparseable_field_refuses_the_whole_entry() {
    assert!(parse_margins(&four("", "10", "10", "10"), PT, &base()).is_none());
    assert!(parse_margins(&four("10", "abc", "10", "10"), PT, &base()).is_none());
    assert!(parse_margins(&four("10", "10", "10", "10"), PT, &base()).is_some());
}

/// **The reseed key changes when the unit does.** The seeded text is written in
/// the unit, so a key that ignored it would leave millimetres sitting under an
/// `in` label after a unit switch — the same failure the size field's key
/// guards against.
#[test]
fn the_reseed_key_tracks_both_the_margins_and_the_unit() {
    let layout = PageLayout::default();
    let mut moved = PageLayout::default();
    moved.margins.left = Points::new(90.0);

    assert_ne!(
        margin_field_key(&layout, PT),
        margin_field_key(&moved, PT),
        "a changed margin did not change the key"
    );
    assert_ne!(
        margin_field_key(&layout, PT),
        margin_field_key(&layout, MM),
        "a changed unit did not change the key"
    );
    assert_eq!(
        margin_field_key(&layout, PT),
        margin_field_key(&PageLayout::default(), PT),
        "the key is not stable for an unchanged layout"
    );
}
