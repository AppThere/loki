// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the app-scoped document defaults (Spec 08 T6.3, D-07).

use super::{DefaultMargins, DefaultPageSize, DocumentDefaults, MAX_CUSTOM_SIZES};
use loki_primitives::units::{MeasurementUnit, Points};

fn size(w: f64, h: f64) -> DefaultPageSize {
    DefaultPageSize {
        width: Points::new(w),
        height: Points::new(h),
    }
}

fn margins(v: f64) -> DefaultMargins {
    DefaultMargins {
        top: Points::new(v),
        bottom: Points::new(v),
        left: Points::new(v),
        right: Points::new(v),
    }
}

#[test]
fn an_empty_file_is_the_no_choice_recorded_state() {
    let d = DocumentDefaults::default();
    assert!(d.page_size.is_none());
    assert!(d.margins.is_none());
    assert!(d.measurement_unit.is_none());
    assert!(d.custom_sizes.is_empty());
}

/// Every field round-trips, and an absent field stays absent — `None` means "no
/// choice recorded", which the seeding path distinguishes from a recorded value
/// that happens to equal the built-in.
#[test]
fn the_settings_round_trip_through_json() {
    let d = DocumentDefaults {
        page_size: Some(size(595.28, 841.89)),
        margins: Some(margins(56.7)),
        measurement_unit: Some(MeasurementUnit::Millimeter),
        custom_sizes: vec![size(500.0, 700.0)],
    };
    let json = serde_json::to_string(&d).expect("serialise");
    let back: DocumentDefaults = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, d);

    // A file written by an older build, missing every field, still loads.
    let sparse: DocumentDefaults = serde_json::from_str("{}").expect("deserialise");
    assert_eq!(sparse, DocumentDefaults::default());
}

/// **D-07's line: geometry here, styles in the document.** A style catalogue
/// stored app-wide would make one document render two ways on two machines,
/// which is exactly what "never embed" exists to prevent. Asserted against the
/// serialised form, because that is what a future field would have to appear in.
#[test]
fn no_style_state_is_persisted() {
    let d = DocumentDefaults {
        page_size: Some(size(612.0, 792.0)),
        margins: Some(margins(72.0)),
        measurement_unit: Some(MeasurementUnit::Inch),
        custom_sizes: vec![size(500.0, 700.0)],
    };
    let json = serde_json::to_string(&d).expect("serialise");
    for forbidden in ["style", "Style", "font", "Font", "paragraph", "catalog"] {
        assert!(
            !json.contains(forbidden),
            "{forbidden:?} reached the app-scoped settings: {json}"
        );
    }
}

/// Implausible values are dropped at load, so no caller has to re-check them.
/// Both ends of both ranges, plus the non-finite cases a hand-edited file can
/// carry.
#[test]
fn implausible_geometry_is_dropped_rather_than_propagated() {
    let bad_sizes = [
        size(0.0, 792.0),
        size(612.0, 0.0),
        size(-612.0, 792.0),
        size(35.0, 792.0),
        size(14401.0, 792.0),
        size(f64::NAN, 792.0),
        size(f64::INFINITY, 792.0),
    ];
    for s in bad_sizes {
        assert!(!s.is_plausible(), "{s:?} was accepted as a page size");
    }
    // ...and the boundary values are usable, so the range is not a lie.
    assert!(size(36.0, 36.0).is_plausible());
    assert!(size(14400.0, 14400.0).is_plausible());

    // Margins: zero is legitimate (full bleed), negative is not.
    assert!(margins(0.0).is_plausible(), "a zero margin was rejected");
    assert!(!margins(-1.0).is_plausible());
    assert!(!margins(f64::NAN).is_plausible());
    assert!(margins(72.0).is_plausible());
}

/// The remembered list exists to bring back sizes the **catalogue cannot name**.
/// Recording a named one would fill it with A4 and Letter, which the picker
/// already offers.
#[test]
fn only_unnamed_sizes_are_remembered() {
    let mut d = DocumentDefaults::default();
    d.remember_custom_size(size(595.28, 841.89), /* is_named */ true);
    assert!(
        d.custom_sizes.is_empty(),
        "a catalogued paper was recorded as a custom size"
    );
    d.remember_custom_size(size(500.0, 700.0), false);
    assert_eq!(d.custom_sizes, vec![size(500.0, 700.0)]);
    // An implausible one is refused even when unnamed.
    d.remember_custom_size(size(0.0, 700.0), false);
    assert_eq!(d.custom_sizes.len(), 1);
}

/// Most recent first, no duplicates, bounded length — re-entering a size moves
/// it to the front rather than adding a second copy.
#[test]
fn remembered_sizes_are_recent_first_deduplicated_and_bounded() {
    let mut d = DocumentDefaults::default();
    for i in 0..(MAX_CUSTOM_SIZES + 3) {
        d.remember_custom_size(size(400.0 + i as f64, 700.0), false);
    }
    assert_eq!(
        d.custom_sizes.len(),
        MAX_CUSTOM_SIZES,
        "the list is unbounded"
    );
    // The newest is first and the oldest fell off.
    assert_eq!(
        d.custom_sizes[0],
        size(400.0 + (MAX_CUSTOM_SIZES + 2) as f64, 700.0)
    );
    assert!(!d.custom_sizes.contains(&size(400.0, 700.0)));

    // Re-entering an existing size promotes rather than duplicates.
    let existing = d.custom_sizes[3];
    d.remember_custom_size(existing, false);
    assert_eq!(d.custom_sizes[0], existing);
    assert_eq!(
        d.custom_sizes.iter().filter(|s| **s == existing).count(),
        1,
        "re-entering a remembered size duplicated it"
    );
}
