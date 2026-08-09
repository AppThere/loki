// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The three computed zooms.

use super::super::{ZOOM_MAX_PERCENT, ZOOM_MIN_PERCENT};
use super::{actual_size_zoom_percent, fit_page_zoom_percent, fit_width_zoom_percent};

/// US Letter, 8.5 × 11 inches, as the CSS pixels the paint path uses.
const LETTER_W: f32 = 816.0; // 612 pt
const LETTER_H: f32 = 1056.0; // 792 pt

/// Fit Width fills the usable width — and **floors**, because a zoom that
/// rounds up produces a page one pixel wider than the space it was fitted to,
/// which is a horizontal scrollbar appearing at the moment the user asked for
/// the page to fit.
#[test]
fn fit_width_fills_the_usable_width_without_overflowing() {
    let z = fit_width_zoom_percent(1000.0, LETTER_W, 40.0).expect("fits");
    assert_eq!(z, 117, "(1000-40)/816 = 1.176…, floored");
    assert!(
        LETTER_W * z as f32 / 100.0 <= 960.0,
        "the fitted page must not exceed the usable width",
    );
}

/// Fit Page takes the **smaller** of the two axes. A page fitted only by width
/// still runs off the bottom, which is not what the command promises — and the
/// wrong `min`/`max` here is invisible on a wide window and obvious on a short
/// one.
#[test]
fn fit_page_is_limited_by_whichever_axis_binds() {
    // A wide, short viewport: height binds.
    let z = fit_page_zoom_percent(2000.0, 600.0, LETTER_W, LETTER_H, 40.0).expect("fits");
    let by_height = fit_width_zoom_percent(600.0, LETTER_H, 40.0).expect("h");
    assert_eq!(z, by_height);
    assert!(LETTER_H * z as f32 / 100.0 <= 560.0, "must fit vertically");

    // A tall, narrow viewport: width binds.
    let z = fit_page_zoom_percent(500.0, 2000.0, LETTER_W, LETTER_H, 40.0).expect("fits");
    assert_eq!(z, fit_width_zoom_percent(500.0, LETTER_W, 40.0).expect("w"));
}

/// **An unmeasured viewport yields no fit, rather than a fit computed from
/// zero.** The first frame reports a zero-size viewport, and a fit derived from
/// it would clamp the document to the bottom of the range on open — arriving as
/// a zoom-out nobody asked for, from a measurement that does not exist yet.
#[test]
fn an_unmeasured_viewport_produces_no_fit() {
    assert_eq!(fit_width_zoom_percent(0.0, LETTER_W, 40.0), None);
    assert_eq!(
        fit_width_zoom_percent(40.0, LETTER_W, 40.0),
        None,
        "all margin"
    );
    assert_eq!(fit_width_zoom_percent(1000.0, 0.0, 40.0), None, "no page");
    assert_eq!(
        fit_page_zoom_percent(1000.0, 0.0, LETTER_W, LETTER_H, 40.0),
        None
    );
}

/// A fit is still a zoom the control offers, so an enormous window cannot fit a
/// page at 1200%.
#[test]
fn a_fit_stays_inside_the_offered_range() {
    assert_eq!(
        fit_width_zoom_percent(20_000.0, LETTER_W, 0.0),
        Some(ZOOM_MAX_PERCENT),
    );
    assert_eq!(
        fit_width_zoom_percent(120.0, LETTER_W, 0.0),
        Some(ZOOM_MIN_PERCENT),
    );
}

/// Actual Size at a display whose CSS pixels are the CSS standard is exactly
/// 100% — the anchor the whole calculation hangs on.
#[test]
fn actual_size_on_a_96_ppi_display_is_one_hundred_percent() {
    assert_eq!(actual_size_zoom_percent(Some(96.0)), Some(100));
}

/// A denser display needs a *higher* zoom for a document inch to measure a
/// physical inch. The inverted ratio is the plausible-looking mistake, and it
/// makes the page smaller on exactly the displays where it should be larger.
#[test]
fn a_denser_display_needs_more_zoom_not_less() {
    let dense = actual_size_zoom_percent(Some(144.0)).expect("144 ppi");
    assert_eq!(dense, 150);
    assert!(dense > 100, "denser must zoom in, got {dense}");

    let sparse = actual_size_zoom_percent(Some(72.0)).expect("72 ppi");
    assert!(sparse < 100, "sparser must zoom out, got {sparse}");
}

/// **No density reported means no Actual Size, not a guess (T5.5).** A
/// substituted default produces a page that is confidently the wrong physical
/// size, which is worse than a command that declines — the reader has no way to
/// tell a wrong ruler from a right one.
#[test]
fn an_unknown_density_declines_rather_than_guessing() {
    assert_eq!(actual_size_zoom_percent(None), None);
    assert_eq!(actual_size_zoom_percent(Some(0.0)), None);
    assert_eq!(actual_size_zoom_percent(Some(f32::NAN)), None);
    assert_eq!(actual_size_zoom_percent(Some(f32::INFINITY)), None);
}
