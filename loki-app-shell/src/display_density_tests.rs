// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What makes a density believable, and the two ways one is obtained.

use super::{
    DensitySource, calibrated_css_ppi, css_ppi_from_physical, device_ppi_from_mm, plausible_ppi,
};

/// A 27-inch 2560×1440 monitor: 596 mm wide, ~109 ppi.
const MONITOR_PX: u32 = 2560;
const MONITOR_MM: u32 = 596;

/// The ordinary case, to a tolerance a ruler could not distinguish.
#[test]
fn a_real_monitor_yields_its_real_density() {
    let ppi = device_ppi_from_mm(MONITOR_PX, MONITOR_MM).expect("plausible");
    assert!((ppi - 109.0).abs() < 1.0, "got {ppi}");
}

/// **The failure this environment actually produced.** XRandR reported 0 mm on
/// the sitting host; the division is then infinite and the zoom that follows is
/// nonsense. `None` is the only honest answer, and it takes the same path as no
/// answer at all.
#[test]
fn a_display_reporting_zero_millimetres_is_rejected() {
    assert_eq!(device_ppi_from_mm(1280, 0), None);
    assert_eq!(device_ppi_from_mm(0, 338), None);
    assert_eq!(device_ppi_from_mm(0, 0), None);
}

/// **The bounds reject nonsense, not unusual hardware (evidence rule 3).** A
/// range tight enough to look tidy would reject a television and a phone —
/// rejecting the phenomenon along with the noise — so both must be accepted.
#[test]
fn the_plausible_range_admits_real_screens_at_both_extremes() {
    // 55-inch 4K television: 3840 px over 1210 mm ≈ 81 ppi.
    assert!(device_ppi_from_mm(3840, 1210).is_some(), "television");
    // Phone: 1440 px over 68 mm ≈ 538 ppi.
    assert!(device_ppi_from_mm(1440, 68).is_some(), "phone");
    // And the polarity: figures no panel has ever had are refused.
    assert!(!plausible_ppi(5.0), "5 ppi is not a screen");
    assert!(!plausible_ppi(5000.0), "5000 ppi is not a screen");
    assert!(!plausible_ppi(f32::NAN));
    assert!(!plausible_ppi(f32::INFINITY));
}

/// **Device pixels are not CSS pixels, and the factor is the scale factor.**
/// This is the 100% error on the 2× display T5.5's acceptance criterion is
/// measured against, which is why the conversion has one home.
#[test]
fn a_retina_display_halves_its_device_density() {
    let device = 220.0;
    let css = css_ppi_from_physical(device, 2.0).expect("2x");
    assert!((css - 110.0).abs() < 0.01, "got {css}");
    assert!(
        css < device,
        "CSS density must be lower than device density on a 2x panel",
    );
    // And at 1× the two are the same number — the case that makes the division
    // invisible if you only ever test on a non-scaled display.
    assert_eq!(css_ppi_from_physical(96.0, 1.0), Some(96.0));
}

/// A scale factor that is not a positive finite number produces no density: a
/// bad divisor gives a bad quotient, and the quotient is what reaches the zoom.
#[test]
fn an_unusable_scale_factor_yields_no_density() {
    assert_eq!(css_ppi_from_physical(220.0, 0.0), None);
    assert_eq!(css_ppi_from_physical(220.0, -2.0), None);
    assert_eq!(css_ppi_from_physical(220.0, f64::NAN), None);
}

/// Calibration scales the assumed density by however wrong the drawn line was.
/// A line the app thinks is 100 mm that measures 80 mm means the display is
/// denser than assumed, so the density goes **up**.
#[test]
fn calibration_scales_the_assumption_by_the_measured_error() {
    let d = calibrated_css_ppi(96.0, 100.0, 80.0).expect("plausible");
    assert!((d.css_px_per_inch - 120.0).abs() < 0.01, "got {d:?}");
    assert_eq!(d.source, DensitySource::Calibrated);

    // The other direction: a line that measures longer than drawn means a
    // sparser display. Asserted because a sign error passes the test above.
    let d = calibrated_css_ppi(96.0, 100.0, 120.0).expect("plausible");
    assert!(d.css_px_per_inch < 96.0, "got {d:?}");
}

/// A perfect measurement leaves the assumption alone — the identity that says
/// the arithmetic is a correction rather than a replacement.
#[test]
fn a_measurement_matching_the_drawing_changes_nothing() {
    let d = calibrated_css_ppi(96.0, 100.0, 100.0).expect("plausible");
    assert!((d.css_px_per_inch - 96.0).abs() < 0.01, "got {d:?}");
}

/// **The wrong-units answer is refused.** Centimetres typed where millimetres
/// were asked for is a factor of ten, and accepting it calibrates the display an
/// order of magnitude out — silently, since nothing else would notice.
#[test]
fn an_implausible_correction_is_refused_rather_than_applied() {
    assert_eq!(calibrated_css_ppi(96.0, 100.0, 10.0), None, "centimetres");
    assert_eq!(calibrated_css_ppi(96.0, 100.0, 1000.0), None, "metres");
    assert_eq!(calibrated_css_ppi(96.0, 100.0, 0.0), None);
    assert_eq!(calibrated_css_ppi(96.0, 100.0, -50.0), None);
    assert_eq!(calibrated_css_ppi(96.0, 0.0, 100.0), None);
    assert_eq!(calibrated_css_ppi(96.0, f32::NAN, 100.0), None);
}

/// The accepted band is wide enough for a real correction. A 2× display whose
/// reader is calibrating from the 96 ppi assumption needs a factor of ~1.15;
/// the boundary must not be so tight that ordinary hardware is refused.
#[test]
fn an_ordinary_correction_is_inside_the_accepted_band() {
    assert!(
        calibrated_css_ppi(96.0, 100.0, 87.0).is_some(),
        "a 2x panel"
    );
    assert!(
        calibrated_css_ppi(96.0, 100.0, 51.0).is_some(),
        "just inside"
    );
    assert!(
        calibrated_css_ppi(96.0, 100.0, 49.0).is_none(),
        "just outside"
    );
}
