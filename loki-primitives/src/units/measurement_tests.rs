// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the measurement-unit resolution chain and its format/parse pair
//! (Spec 08 T6.4, decision D-03).

use super::{MeasurementUnit, resolve_measurement_unit};
use crate::units::well_known::Points;

/// **The chain, at every rung, in both polarities.** Each source is tested
/// where it decides *and* where it is overruled — a test that only supplied one
/// source at a time would pass for any ordering of the four.
#[test]
fn the_resolution_order_is_explicit_then_os_then_locale_then_metric() {
    let ex = Some(MeasurementUnit::Pica);

    // Explicit beats everything, including an OS setting and a locale that both
    // say something else.
    assert_eq!(
        resolve_measurement_unit(ex, Some("2"), Some("en_US.UTF-8")),
        MeasurementUnit::Pica
    );
    // OS setting beats the locale region.
    assert_eq!(
        resolve_measurement_unit(None, Some("1"), Some("en_US.UTF-8")),
        MeasurementUnit::Millimeter,
        "LC_MEASUREMENT=1 (metric) did not overrule a US locale"
    );
    assert_eq!(
        resolve_measurement_unit(None, Some("2"), Some("de_DE.UTF-8")),
        MeasurementUnit::Inch,
        "LC_MEASUREMENT=2 (US) did not overrule a German locale"
    );
    // Locale region decides when the OS setting is absent or unusable.
    assert_eq!(
        resolve_measurement_unit(None, None, Some("en_US.UTF-8")),
        MeasurementUnit::Inch
    );
    assert_eq!(
        resolve_measurement_unit(None, Some("garbage"), Some("en_US.UTF-8")),
        MeasurementUnit::Inch,
        "an unparseable LC_MEASUREMENT should fall through, not decide"
    );
    // Metric ends the chain.
    assert_eq!(
        resolve_measurement_unit(None, None, None),
        MeasurementUnit::Millimeter
    );
}

/// A locale with no region says nothing — falling through to metric is D-03's
/// default rather than an inference that an unqualified `en` speaker is
/// American.
#[test]
fn a_locale_without_a_region_does_not_decide() {
    for locale in ["C", "POSIX", "en", ""] {
        assert_eq!(
            resolve_measurement_unit(None, None, Some(locale)),
            MeasurementUnit::Millimeter,
            "{locale:?} was read as a region"
        );
    }
}

#[test]
fn the_three_non_metric_countries_are_imperial_and_others_are_not() {
    for locale in ["en_US.UTF-8", "en_LR", "my_MM"] {
        assert_eq!(
            resolve_measurement_unit(None, None, Some(locale)),
            MeasurementUnit::Inch,
            "{locale} should be imperial"
        );
    }
    for locale in ["en_GB.UTF-8", "de_DE", "ja_JP", "fr_CA", "es_MX"] {
        assert_eq!(
            resolve_measurement_unit(None, None, Some(locale)),
            MeasurementUnit::Millimeter,
            "{locale} should be metric"
        );
    }
}

/// **The two locale tables are answering different questions.** Mexico takes US
/// Letter paper and metric measurements; a future tidy-up that merged this
/// region list with `default_page_size_for_locale`'s would get one of the two
/// wrong for Mexico, Canada and the Philippines alike.
#[test]
fn mexico_uses_letter_paper_but_metric_units() {
    assert_eq!(
        resolve_measurement_unit(None, None, Some("es_MX.UTF-8")),
        MeasurementUnit::Millimeter
    );
    // The paper-size half of the claim is asserted in `loki-doc-model`, which
    // owns that table; the point here is that this one must not adopt it.
    assert!(
        !super::IMPERIAL_REGIONS.contains(&"_MX"),
        "the measurement table has taken on the paper-size table's regions"
    );
    for letter_paper_but_metric in ["_MX", "_CA", "_PH"] {
        assert!(
            !super::IMPERIAL_REGIONS.contains(&letter_paper_but_metric),
            "{letter_paper_but_metric} is a US-Letter region, not an imperial-measure one"
        );
    }
}

/// Formatting then parsing must return the same length, or switching the
/// display unit would silently edit every value on screen.
#[test]
fn format_and_parse_round_trip_in_every_unit() {
    for unit in MeasurementUnit::ALL.iter().copied() {
        for pt in [1.0_f64, 36.0, 72.0, 595.28, 841.89, 1440.0] {
            let shown = unit.format_bare(Points::new(pt));
            let back = unit.parse(&shown).expect("re-parses");
            let drift = (back.value() - pt).abs();
            assert!(
                drift < 0.5,
                "{unit:?}: {pt} pt showed as {shown:?} and came back as {} pt \
                 ({drift:.3} pt of drift — the display rounding is too coarse)",
                back.value()
            );
        }
    }
}

/// The conversions themselves, against values whose answers are definitional.
#[test]
fn the_conversions_are_the_defined_ones() {
    let inch = Points::new(72.0);
    assert!((MeasurementUnit::Millimeter.value_of(inch) - 25.4).abs() < 1e-9);
    assert!((MeasurementUnit::Centimeter.value_of(inch) - 2.54).abs() < 1e-9);
    assert!((MeasurementUnit::Inch.value_of(inch) - 1.0).abs() < 1e-9);
    assert!((MeasurementUnit::Point.value_of(inch) - 72.0).abs() < 1e-9);
    assert!((MeasurementUnit::Pica.value_of(inch) - 6.0).abs() < 1e-9);

    // ...and back again.
    assert!((MeasurementUnit::Millimeter.to_points(25.4).value() - 72.0).abs() < 1e-9);
    assert!((MeasurementUnit::Pica.to_points(6.0).value() - 72.0).abs() < 1e-9);
}

#[test]
fn a_bare_number_is_read_in_the_active_unit() {
    assert!(
        (MeasurementUnit::Inch.parse("1").expect("parses").value() - 72.0).abs() < 1e-9,
        "a bare 1 with inches active should be one inch"
    );
    assert!(
        (MeasurementUnit::Millimeter
            .parse("25.4")
            .expect("parses")
            .value()
            - 72.0)
            .abs()
            < 1e-9
    );
}

/// An explicit suffix overrides the active unit — the same number typed with
/// and without one must differ, or the suffix is being ignored.
#[test]
fn an_explicit_suffix_overrides_the_active_unit() {
    let active = MeasurementUnit::Millimeter;
    let bare = active.parse("1").expect("parses");
    let suffixed = active.parse("1in").expect("parses");
    assert!((bare.value() - active.to_points(1.0).value()).abs() < 1e-9);
    assert!((suffixed.value() - 72.0).abs() < 1e-9);
    assert!(
        (bare.value() - suffixed.value()).abs() > 1.0,
        "the `in` suffix was ignored"
    );
    // Spacing and case are both tolerated.
    assert!((active.parse(" 1 IN ").expect("parses").value() - 72.0).abs() < 1e-9);
    assert!((active.parse("12 pt").expect("parses").value() - 12.0).abs() < 1e-9);
    assert!((active.parse("1pc").expect("parses").value() - 12.0).abs() < 1e-9);
}

/// Every rejection polarity. A parser exercised only on what it accepts reports
/// nothing about what it lets through.
#[test]
fn unparseable_entries_are_rejected() {
    let u = MeasurementUnit::Millimeter;
    for bad in [
        "",
        "   ",
        "abc",
        "mm",
        "1.2.3",
        "1 furlong",
        "1 px",
        "inf",
        "NaN",
        "--5",
    ] {
        assert!(u.parse(bad).is_none(), "accepted {bad:?}");
    }
    // A negative number parses — rejecting it is the caller's range rule, not
    // the unit's, and the page-size field already has one.
    assert!(u.parse("-5").is_some());
}

#[test]
fn abbreviations_are_unique_so_a_suffix_is_unambiguous() {
    for (i, a) in MeasurementUnit::ALL.iter().enumerate() {
        for b in MeasurementUnit::ALL.iter().skip(i + 1) {
            assert_ne!(
                a.abbreviation(),
                b.abbreviation(),
                "{a:?} and {b:?} share a suffix, so parsing it would depend on list order"
            );
        }
    }
}
