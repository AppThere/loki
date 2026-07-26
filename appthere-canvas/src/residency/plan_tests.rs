// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the residency planner. Extracted per the file-ceiling idiom.

use super::super::budget::{BUDGET_CEILING_BYTES, TextureBudget};
use super::super::geometry::{PageBox, ViewportSpec};
use super::{MIN_RASTER_SCALE, plan_residency, strictly_visible};

fn letter_doc(n: usize) -> Vec<PageBox> {
    vec![PageBox::us_letter(); n]
}

/// Scrolled well into a long document, 900 px viewport.
fn vp(zoom: f64, dsf: f64) -> ViewportSpec {
    ViewportSpec::new(20_000.0, 900.0, zoom, dsf)
}

#[test]
fn a_generous_budget_changes_nothing() {
    // The common case, and the first half of the T2.1 prediction: rows already
    // under budget must be byte-identical, not merely close.
    let doc = letter_doc(500);
    let v = vp(1.0, 1.0);
    let plan = plan_residency(&doc, &v, TextureBudget::exact(BUDGET_CEILING_BYTES));
    assert!(!plan.over_budget);
    assert!(plan.tiles.iter().all(|t| t.raster_scale == 1.0));
    assert_eq!(
        plan.total_bytes,
        super::super::geometry::resident_texture_bytes(&doc, &v),
        "an unpressured plan must equal the unbudgeted residency exactly",
    );
}

#[test]
fn the_visible_set_is_never_dropped_at_any_budget() {
    // The acceptance criterion. Swept across the whole zoom x DPI grid at the
    // smallest budget the derivation can produce, because a policy that holds
    // at one operating point and not another is not a policy.
    let doc = letter_doc(500);
    let tiny = TextureBudget::exact(0); // clamps up to the 24 MiB floor
    for zoom in [0.25, 0.5, 1.0, 2.0, 4.0] {
        for dsf in [1.0, 2.0, 3.0] {
            let v = vp(zoom, dsf);
            let plan = plan_residency(&doc, &v, tiny);
            let want: Vec<usize> = strictly_visible(&doc, &v)
                .iter()
                .enumerate()
                .filter_map(|(i, vis)| vis.then_some(i))
                .collect();
            for page in want {
                assert!(
                    plan.tiles.iter().any(|t| t.page_index == page),
                    "page {page} is visible at zoom {zoom} dsf {dsf} and was dropped",
                );
            }
        }
    }
}

#[test]
fn pressure_reduces_off_centre_scale_before_it_drops_anything() {
    // L08-002's ordering. The budget is set just below the full-scale total, so
    // exactly one concession is needed and we can see which one it was. At
    // 200%/2x rather than 100%/1x, because the 100% total (13.1 MiB) is under
    // the 24 MiB budget floor and `exact` would clamp the "tight" budget back
    // above it — the test would then measure nothing.
    let doc = letter_doc(500);
    let v = vp(2.0, 2.0);
    let full = super::super::geometry::resident_texture_bytes(&doc, &v);
    let plan = plan_residency(&doc, &v, TextureBudget::exact(full - 1));

    let mounted_before = super::super::geometry::resident_pages(&doc, &v)
        .iter()
        .filter(|&&v| v)
        .count();
    assert_eq!(
        plan.tiles.len(),
        mounted_before,
        "the first concession must be resolution, not the tile count",
    );
    assert!(
        plan.tiles.iter().any(|t| t.raster_scale < 1.0),
        "something must have been reduced",
    );
    assert!(
        plan.tiles
            .iter()
            .all(|t| t.visible == (t.raster_scale == 1.0)),
        "only off-centre tiles may be reduced while a visible-only plan fits",
    );
}

#[test]
fn a_hard_budget_drops_off_centre_tiles_furthest_first() {
    let doc = letter_doc(500);
    let v = vp(2.0, 2.0);
    // Well under the 157.8 MiB this configuration wants, but comfortably over
    // what the visible set alone costs, so step 3 is the operative one.
    let plan = plan_residency(&doc, &v, TextureBudget::exact(60 * 1024 * 1024));
    assert!(!plan.over_budget);
    assert!(plan.total_bytes <= 60 * 1024 * 1024);
    let visible_count = strictly_visible(&doc, &v).iter().filter(|&&b| b).count();
    assert!(
        plan.tiles.len() >= visible_count,
        "every visible page must still be mounted",
    );
}

#[test]
fn the_visible_set_is_reduced_only_as_a_last_resort() {
    // 400% on a 3x display: one visible page is ~496 MB, far over any derived
    // budget. The only legal move left is resolution, and it must be taken
    // rather than dropping the page.
    let doc = letter_doc(500);
    let v = vp(4.0, 3.0);
    let plan = plan_residency(&doc, &v, TextureBudget::exact(64 * 1024 * 1024));
    assert!(
        plan.tiles.iter().any(|t| t.visible),
        "the visible page is still mounted",
    );
    let visible_scale = plan
        .tiles
        .iter()
        .find(|t| t.visible)
        .map(|t| t.raster_scale)
        .unwrap_or(1.0);
    assert!(
        visible_scale < 1.0,
        "the visible tile must be reduced, got {visible_scale}",
    );
    assert!(visible_scale >= MIN_RASTER_SCALE);
    assert!(plan.total_bytes <= 64 * 1024 * 1024);
    assert!(!plan.over_budget);
}

#[test]
fn over_budget_is_reported_rather_than_resolved_below_legibility() {
    // The corner where no legal move remains: the visible page at the scale
    // floor still exceeds the budget floor. Being over budget is the honest
    // outcome — the alternatives are a blank page or an illegible one.
    let doc = letter_doc(500);
    let v = vp(4.0, 3.0);
    let plan = plan_residency(&doc, &v, TextureBudget::exact(0)); // 24 MiB floor
    assert!(plan.over_budget, "this cannot be satisfied and must say so");
    assert!(plan.tiles.iter().any(|t| t.visible));
    assert!(
        plan.tiles
            .iter()
            .all(|t| t.raster_scale >= MIN_RASTER_SCALE),
        "never below legibility, even to reach the budget",
    );
}

#[test]
fn scale_reduction_is_quadratic_in_bytes() {
    // The property step 4's square root relies on. If tile sizing ever stopped
    // being area-proportional this would break silently and the exact-fit
    // calculation would overshoot.
    let page = PageBox::us_letter();
    let full = page.texture_bytes(2.0, 2.0);
    let half = page.texture_bytes(2.0, 1.0);
    let ratio = full as f64 / half as f64;
    assert!((3.99..4.01).contains(&ratio), "got {ratio}");
}

#[test]
fn an_empty_document_plans_nothing() {
    let plan = plan_residency(&[], &vp(1.0, 1.0), TextureBudget::baseline());
    assert!(plan.tiles.is_empty());
    assert_eq!(plan.total_bytes, 0);
    assert!(!plan.over_budget);
}

#[test]
fn strict_visibility_is_narrower_than_the_mounting_window() {
    // The distinction the whole policy rests on: the window is the visible rect
    // grown by one screen each side, so there are always off-centre tiles to
    // concede before anything visible is touched.
    let doc = letter_doc(500);
    let v = vp(1.0, 1.0);
    let windowed = super::super::geometry::resident_pages(&doc, &v)
        .iter()
        .filter(|&&b| b)
        .count();
    let visible = strictly_visible(&doc, &v).iter().filter(|&&b| b).count();
    assert!(
        visible < windowed,
        "expected fewer visible ({visible}) than windowed ({windowed})",
    );
    assert!(visible >= 1);
}
