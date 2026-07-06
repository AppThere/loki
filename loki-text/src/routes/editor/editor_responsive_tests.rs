// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the pure page-fit helpers in `editor_responsive`.
//!
//! These lock in the 4a.4 "real `Viewport::zoom`" wiring: the page-fit renderer
//! decision must scale with the user's zoom, so the *same* measured width can
//! flip between paginated and reflow as the zoom changes. Before the wiring the
//! zoom was hardcoded to 100% and these would all resolve to `Paginated`.

use loki_renderer::ViewMode;

use super::{desired_view_mode, zoom_fraction};

// A4 default page width in CSS px (matches `appthere_ui::tokens::PAGE_WIDTH_PX`).
const PAGE: f32 = 794.0;

#[test]
fn zoom_fraction_maps_percent_to_fraction() {
    assert_eq!(zoom_fraction(50), 0.5);
    assert_eq!(zoom_fraction(100), 1.0);
    assert_eq!(zoom_fraction(150), 1.5);
    assert_eq!(zoom_fraction(200), 2.0);
}

#[test]
fn same_width_stays_paginated_at_100_percent() {
    // 1000 px comfortably fits a 794 px page + gutters at 100% zoom.
    assert_eq!(
        desired_view_mode(1000.0, PAGE, 100, false),
        ViewMode::Paginated,
    );
}

#[test]
fn zooming_in_flips_a_fitting_page_to_reflow() {
    // The regression this wiring fixes: at 200% zoom the same 1000 px viewport
    // can no longer show a full page column, so a currently-paginated editor
    // must fall back to reflow rather than force horizontal scrolling.
    assert_eq!(desired_view_mode(1000.0, PAGE, 200, true), ViewMode::Reflow,);
}

#[test]
fn zooming_out_lets_a_page_fit_again() {
    // A narrow 900 px viewport that cannot fit the page at 100% (needs
    // 794 + 48 gutter + 48 hysteresis = 890 to switch *in* from reflow — it just
    // fits) fits with room to spare once zoomed out to 75%.
    assert_eq!(
        desired_view_mode(700.0, PAGE, 100, false),
        ViewMode::Reflow,
        "700 px cannot fit a full page at 100%",
    );
    assert_eq!(
        desired_view_mode(700.0, PAGE, 75, false),
        ViewMode::Paginated,
        "the same 700 px fits the shrunk page at 75%",
    );
}

#[test]
fn hysteresis_holds_the_current_mode_near_the_boundary() {
    // At 100% a page needs 794 + 48 = 842 px. Within the ±48 px dead-band the
    // current mode is sticky: 860 px (needed .. needed+hyst) neither switches in
    // from reflow nor out from paginated.
    assert_eq!(
        desired_view_mode(860.0, PAGE, 100, true),
        ViewMode::Paginated,
        "stays paginated: 860 >= 842 - 48",
    );
    assert_eq!(
        desired_view_mode(860.0, PAGE, 100, false),
        ViewMode::Reflow,
        "stays reflow: 860 < 842 + 48",
    );
}
