// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the reading measure (Spec 08 T7.2 / D-05).
//!
//! Every assertion here is a **relation** between two measurements, never an
//! absolute number of points: the absolute answer depends on which fonts the
//! host has, and a test that pinned one would pass on this machine and fail on
//! the next. The relations are what the feature claims.

use super::{
    DEFAULT_MEASURE_CHARS, MAX_MEASURE_CHARS, MIN_MEASURE_CHARS, mean_advance_pt, measure_width_pt,
};
use crate::font::FontResources;

/// Font resources with a known face registered, so the measurement has
/// something to shape. Returns `None` when the host has neither test font — the
/// tests then skip rather than assert against an empty font collection, which
/// would be a null result dressed as a pass.
fn resources() -> Option<FontResources> {
    let mut r = FontResources::with_bundled_fonts_only();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
            return Some(r);
        }
    }
    None
}

/// The mean advance is a real, plausible fraction of the em — the sanity bound
/// that catches a measurement returning the whole sample's width, or one point,
/// or the character count.
#[test]
fn the_mean_advance_is_a_fraction_of_the_em() {
    let Some(mut r) = resources() else { return };
    let mean = mean_advance_pt(&mut r, "Liberation Sans", 12.0).expect("measurable");
    assert!(
        mean > 12.0 * 0.2 && mean < 12.0 * 0.9,
        "mean advance {mean} pt is not a plausible fraction of a 12 pt em — \
         a whole-sample width or a per-em value would land outside this"
    );
}

/// **The measure scales with the font size.** This is the half that makes it
/// "live metrics" rather than a constant: the same character count at twice the
/// size is twice the width.
#[test]
fn the_measure_scales_with_the_font_size() {
    let Some(mut r) = resources() else { return };
    let at12 = measure_width_pt(&mut r, "Liberation Sans", 12.0, 76).expect("measurable");
    let at24 = measure_width_pt(&mut r, "Liberation Sans", 24.0, 76).expect("measurable");
    let ratio = at24 / at12;
    assert!(
        (ratio - 2.0).abs() < 0.05,
        "doubling the size changed the measure by {ratio}×, not 2× — the \
         measurement is not tracking the size"
    );
}

/// **The measure scales with the character count**, linearly. A measure that
/// ignored `chars` would be the constant this task exists to remove.
#[test]
fn the_measure_scales_with_the_character_count() {
    let Some(mut r) = resources() else { return };
    let at40 = measure_width_pt(&mut r, "Liberation Sans", 12.0, 40).expect("measurable");
    let at80 = measure_width_pt(&mut r, "Liberation Sans", 12.0, 80).expect("measurable");
    let ratio = at80 / at40;
    assert!(
        (ratio - 2.0).abs() < 0.01,
        "doubling the character count changed the measure by {ratio}×, not 2×"
    );
    assert!(at40 < at80, "the measure did not grow with the count");
}

/// A stored setting outside the range is brought into it rather than refused —
/// and both ends are clamped, not just the one a caller happened to test.
#[test]
fn an_out_of_range_measure_is_clamped_at_both_ends() {
    let Some(mut r) = resources() else { return };
    let mut m =
        |chars| measure_width_pt(&mut r, "Liberation Sans", 12.0, chars).expect("measurable");

    assert_eq!(m(0), m(MIN_MEASURE_CHARS), "the floor was not applied");
    assert_eq!(m(1), m(MIN_MEASURE_CHARS));
    assert_eq!(
        m(u32::MAX),
        m(MAX_MEASURE_CHARS),
        "the ceiling was not applied"
    );
    // And the inverse: a value inside the range is *not* clamped, so the
    // assertions above are about the bounds and not about everything.
    assert!(
        m(DEFAULT_MEASURE_CHARS) > m(MIN_MEASURE_CHARS)
            && m(DEFAULT_MEASURE_CHARS) < m(MAX_MEASURE_CHARS),
        "an in-range measure was clamped"
    );
}

/// An unusable size is reported as "no measure" rather than as a number. The
/// caller keeps what it had; substituting a guess is how a bad setting becomes
/// a bad layout nobody can trace.
#[test]
fn an_unmeasurable_request_returns_none() {
    let Some(mut r) = resources() else { return };
    for bad in [0.0f32, -12.0, f32::NAN, f32::INFINITY] {
        assert!(
            mean_advance_pt(&mut r, "Liberation Sans", bad).is_none(),
            "a size of {bad} produced a measure"
        );
        assert!(measure_width_pt(&mut r, "Liberation Sans", bad, 76).is_none());
    }
    // The inverse: a good size does produce one, so `None` is not the only
    // answer this function knows how to give.
    assert!(mean_advance_pt(&mut r, "Liberation Sans", 12.0).is_some());
}

/// **The measure follows the font that is actually used.** An unknown family
/// resolves through the same fallback the document text does, so it still
/// measures — the alternative (refusing, or measuring a family nothing renders)
/// would give a column width unrelated to the page.
#[test]
fn an_unknown_family_still_measures_through_the_same_fallback() {
    let Some(mut r) = resources() else { return };
    let unknown = mean_advance_pt(&mut r, "No Such Face At All", 12.0);
    assert!(
        unknown.is_some(),
        "an unresolvable family produced no measure at all"
    );
}

/// The default sits inside the band T7.2 states, and inside the accepted range.
/// A default outside its own documented band is the kind of drift a constant
/// invites.
#[test]
fn the_default_measure_is_inside_the_stated_band() {
    assert!(
        (72..=80).contains(&DEFAULT_MEASURE_CHARS),
        "the default {DEFAULT_MEASURE_CHARS} is outside T7.2's 72-80 band"
    );
    assert!(MIN_MEASURE_CHARS < DEFAULT_MEASURE_CHARS);
    assert!(DEFAULT_MEASURE_CHARS < MAX_MEASURE_CHARS);
}
