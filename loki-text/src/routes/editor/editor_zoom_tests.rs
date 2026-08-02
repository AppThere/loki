// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The editor's fit measurements. The arithmetic is `appthere_ui`'s and tested
//! there; what these hold is the wiring — which measurement goes to which
//! argument, which is exactly where a fit silently becomes the wrong one.

use super::{FitInputs, actual_size_percent, fit_page_percent, fit_width_percent};

/// US Letter as the layout reports it: CSS px at 100% zoom.
const LETTER_W_PX: f32 = 816.0;
const LETTER_H_PX: f32 = 1056.0;

fn inputs(w: f32, h: f32) -> FitInputs {
    FitInputs {
        page_width_px: LETTER_W_PX,
        page_height_px: LETTER_H_PX,
        viewport_width_px: w,
        viewport_height_px: h,
    }
}

/// A fit produces a zoom at which the page actually fits, margin included.
#[test]
fn fit_width_produces_a_page_that_fits_the_measured_width() {
    let vp = 1000.0_f32;
    let z = fit_width_percent(inputs(vp, 800.0)).expect("measured");
    let page_px = LETTER_W_PX * z as f32 / 100.0;
    assert!(
        page_px <= vp - 48.0 + 1.0,
        "page {page_px}px does not fit {vp}px less the gutter",
    );
}

/// **The axes are not swapped.** Passing height where width belongs is the
/// wiring mistake this module can make, and on a landscape window it produces a
/// plausible-looking number — so the test uses a viewport whose axes differ
/// enough that a swap changes the answer.
#[test]
fn the_viewport_axes_reach_their_own_arguments() {
    let wide = fit_width_percent(inputs(1600.0, 400.0)).expect("measured");
    let tall = fit_width_percent(inputs(400.0, 1600.0)).expect("measured");
    assert!(
        wide > tall,
        "fit-width must follow the width: {wide} vs {tall}",
    );
}

/// Fit Page is never larger than Fit Width — it has one more constraint, and
/// this is the assertion that fails if the two are wired to the same axis.
#[test]
fn fit_page_never_exceeds_fit_width() {
    for (w, h) in [(1000.0, 700.0), (1600.0, 400.0), (500.0, 2000.0)] {
        let fw = fit_width_percent(inputs(w, h)).expect("w");
        let fp = fit_page_percent(inputs(w, h)).expect("p");
        assert!(fp <= fw, "fit-page {fp} > fit-width {fw} at {w}×{h}");
    }
}

/// **Before the canvas is measured there is no fit.** The first frame reports a
/// zero viewport, and a fit computed from it would zoom the document out on
/// open — from a measurement that does not exist yet.
#[test]
fn an_unmeasured_canvas_yields_no_fit() {
    assert_eq!(fit_width_percent(inputs(0.0, 0.0)), None);
    assert_eq!(fit_page_percent(inputs(0.0, 0.0)), None);
    assert_eq!(
        fit_width_percent(inputs(40.0, 800.0)),
        None,
        "a viewport smaller than the gutter is not a measurement",
    );
}

/// Actual Size declines rather than guessing when the density is unknown, and
/// tracks it when it is known. Both polarities, because the failure modes are
/// opposite: a guess gives a confidently wrong physical size, and a permanent
/// `None` hides a capability that works.
#[test]
fn actual_size_follows_the_reported_density() {
    assert_eq!(actual_size_percent(None), None);
    assert_eq!(actual_size_percent(Some(96.0)), Some(100));
    assert_eq!(actual_size_percent(Some(192.0)), Some(200));
}
